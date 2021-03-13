mod buffer;
mod pipeline;
pub mod queue;

use cgmath::{Matrix2, Rad, Vector2, Vector3};
use log::*;

use std::{
    borrow::Borrow,
    iter,
    mem::{swap, ManuallyDrop},
    ops::Div,
    pin::Pin,
    ptr,
};

use gfx_hal::{
    adapter::Adapter,
    command::{
        ClearColor, ClearValue, CommandBuffer, CommandBufferFlags, Level, RenderAttachmentInfo,
        SubpassContents,
    },
    device::Device,
    format::{ChannelType, Format, SurfaceType},
    image::Layout,
    pass::{Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, SubpassDesc},
    pool::{CommandPool, CommandPoolCreateFlags},
    prelude::{CommandQueue, PhysicalDevice},
    pso::{Rect, Viewport},
    window::{AcquireError, Extent2D, PresentationSurface, Surface, SwapchainConfig},
    Backend, Features, Instance,
};

use pipeline::{create_pipeline, VertexData};

use self::{
    buffer::{Buffer, BufferManager, VertexBuffer},
    queue::{QueueFamilies, Queues},
};

#[derive(Debug)]
pub enum RendererError {
    QueueFamiliesNotFinished,
    QueueGroupMismatch,
    AdapterMismatch,
}

pub struct GearsRenderer<B: Backend> {
    command_buffers: Vec<B::CommandBuffer>,
    command_pools: Vec<B::CommandPool>,
    submit_fences: Vec<B::Fence>,
    submit_semaphores: Vec<B::Semaphore>,

    vertex_buffer: ManuallyDrop<VertexBuffer<B>>,
    pipeline: ManuallyDrop<B::GraphicsPipeline>,

    render_pass: ManuallyDrop<B::RenderPass>,
    framebuffer: ManuallyDrop<B::Framebuffer>,
    surface: ManuallyDrop<B::Surface>,

    buffer_manager: BufferManager,
    queues: Pin<Box<Queues<B>>>,
    device: B::Device,
    adapter: Adapter<B>,
    instance: B::Instance,

    format: Format,
    pub dimensions: Extent2D,
    viewport: Viewport,
    frame: usize,
    frames_in_flight: usize,
    frame_counter: usize,
    frame_counter_tp: instant::Instant,
}

impl<B: Backend> GearsRenderer<B> {
    pub fn new(
        instance: B::Instance,
        mut surface: B::Surface,
        adapter: Adapter<B>,
        queue_families: QueueFamilies,
        extent: Extent2D,
    ) -> Self {
        debug!("Renderer created");

        // device

        let physical_device = &adapter.physical_device;
        let sparsely_bound = physical_device
            .features()
            .contains(Features::SPARSE_BINDING | Features::SPARSE_RESIDENCY_IMAGE_2D);
        let gpu = unsafe {
            physical_device.open(
                &queue_families.get_vec(&adapter).unwrap()[..],
                if sparsely_bound {
                    Features::SPARSE_BINDING | Features::SPARSE_RESIDENCY_IMAGE_2D
                } else {
                    Features::empty()
                },
            )
        }
        .unwrap();

        let queues = queue_families.get_queues(gpu.queue_groups).unwrap();
        let device = gpu.device;

        // swapchain

        let caps = surface.capabilities(&adapter.physical_device);
        let format =
            surface
                .supported_formats(physical_device)
                .map_or(Format::Rgba8Unorm, |formats| {
                    formats
                        .iter()
                        .find(|format| {
                            format.base_format().1 == ChannelType::Unorm
                                && format.base_format().0 == SurfaceType::B8_G8_R8_A8
                        })
                        .cloned()
                        .unwrap_or(formats[0])
                });
        debug!(
            "format chosen: {:?} from {:?}",
            format,
            surface.supported_formats(physical_device)
        );
        let config = SwapchainConfig::from_caps(&caps, format, extent);
        let framebuffer_attachment = config.framebuffer_attachment();
        let extent = extent;
        unsafe {
            surface
                .configure_swapchain(&device, config)
                .expect("Could not configure the swapchain")
        };

        let viewport = Viewport {
            rect: Rect {
                x: 0,
                y: 0,
                w: extent.width as i16,
                h: extent.height as i16,
            },
            depth: 0.0..1.0,
        };

        let render_pass = {
            let attachment = Attachment {
                format: Some(format),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::Store),
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::Undefined..Layout::Present,
            };

            let subpass = SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            ManuallyDrop::new(
                unsafe {
                    device.create_render_pass(
                        std::iter::once(attachment),
                        std::iter::once(subpass),
                        std::iter::empty(),
                    )
                }
                .expect("Could not create a render pass"),
            )
        };

        let framebuffer = ManuallyDrop::new(unsafe {
            device
                .create_framebuffer(
                    &render_pass,
                    iter::once(framebuffer_attachment),
                    extent.to_extent(),
                )
                .expect("Could not create a framebuffer")
        });

        // graphics pipeline
        let pipeline = ManuallyDrop::new(create_pipeline::<B, VertexData>(&device, &render_pass));

        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let mut buffer_manager = BufferManager::new();
        let vertex_buffer = ManuallyDrop::new(VertexBuffer::new::<VertexData>(
            &device,
            &mut buffer_manager,
            &memory_types,
            3,
        ));

        // command pool for every 'frame in flight'
        let frames_in_flight = 3;
        let submit_semaphores = (0..frames_in_flight)
            .map(|_| {
                device
                    .create_semaphore()
                    .expect("Could not create a semaphore")
            })
            .collect::<Vec<_>>();
        let submit_fences = (0..frames_in_flight)
            .map(|_| device.create_fence(true).expect("Could not create a fence"))
            .collect::<Vec<_>>();
        let mut command_pools = (0..frames_in_flight)
            .map(|_| unsafe {
                device
                    .create_command_pool(
                        queues.graphics.as_ref().family,
                        CommandPoolCreateFlags::empty(),
                    )
                    .expect("Could not create a command pool")
            })
            .collect::<Vec<_>>();
        let command_buffers = (0..frames_in_flight)
            .map(|i| unsafe { command_pools[i].allocate_one(Level::Primary) })
            .collect::<Vec<_>>();

        Self {
            command_buffers,
            command_pools,
            submit_fences,
            submit_semaphores,

            vertex_buffer,
            pipeline,

            render_pass,
            framebuffer,
            surface: ManuallyDrop::new(surface),

            buffer_manager,
            queues,
            device,
            adapter,
            instance,

            format,
            dimensions: extent,
            viewport,
            frame: 0,
            frames_in_flight,
            frame_counter: 0,
            frame_counter_tp: instant::Instant::now(),
        }
    }

    pub fn render(&mut self) {
        // acquire the next image from the swapchain
        let surface_image = match unsafe { self.surface.acquire_image(!0) } {
            Ok((image, _)) => image,
            Err(AcquireError::SurfaceLost(_)) => {
                error!("Swapchain surface was lost (display disconnected?)");
                panic!();
            }
            Err(AcquireError::DeviceLost(_)) => {
                error!("Device was lost (GPU disconnected?)");
                panic!();
            }
            Err(_) => {
                self.recreate_swapchain();
                return;
            }
        };

        let rotation_matrix_trig = Matrix2::<f32>::from_angle(Rad {
            0: std::f32::consts::PI * 2.0 / 3.0,
        });

        let rotation_matrix_time = Matrix2::<f32>::from_angle(Rad {
            0: (self.frame as f32) / 50.0,
        });

        let vert_a = rotation_matrix_time * rotation_matrix_trig * Vector2::new(0.0, -0.8);
        let vert_b = rotation_matrix_trig * vert_a;
        let vert_c = rotation_matrix_trig * vert_b;

        let vertices = [
            VertexData {
                position: vert_a,
                color: Vector3::new(1.0, 0.0, 0.0),
            },
            VertexData {
                position: vert_b,
                color: Vector3::new(0.0, 1.0, 0.0),
            },
            VertexData {
                position: vert_c,
                color: Vector3::new(0.0, 0.0, 1.0),
            },
        ];
        self.vertex_buffer
            .write(&self.device, &mut self.buffer_manager, 0, &vertices);

        let frame = self.frame % self.frames_in_flight;
        self.frame += 1;
        self.frame_counter += 1;

        let fence = unsafe {
            let fence = &mut self.submit_fences[frame];
            self.device
                .wait_for_fence(fence, !0)
                .expect("Failed to wait for fence");
            self.device
                .reset_fence(fence)
                .expect("Failed to reset fence");
            self.command_pools[frame].reset(false);
            fence
        };

        // print average fps every 3 seconds
        let avg_fps_interval = instant::Duration::from_secs_f32(3.0);
        if self.frame_counter_tp.elapsed() > avg_fps_interval {
            let time_per_frame = avg_fps_interval.div(self.frame_counter as u32);
            debug!(
                "Average frametime: {:?} ms ({} fps)",
                time_per_frame.as_millis(),
                self.frame_counter
            );
            self.frame_counter = 0;
            self.frame_counter_tp = instant::Instant::now();
        }

        // Rendering
        unsafe {
            let command_buffer = &mut self.command_buffers[frame];
            command_buffer.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

            command_buffer.set_viewports(0, iter::once(self.viewport.clone()));
            command_buffer.set_scissors(0, iter::once(self.viewport.rect));
            command_buffer.bind_graphics_pipeline(&self.pipeline);

            self.vertex_buffer.bind(command_buffer);

            command_buffer.begin_render_pass(
                &self.render_pass,
                &self.framebuffer,
                self.viewport.rect,
                iter::once(RenderAttachmentInfo {
                    image_view: surface_image.borrow(),
                    clear_value: ClearValue {
                        color: ClearColor {
                            float32: [0.18, 0.18, 0.2, 1.0],
                        },
                    },
                }),
                SubpassContents::Inline,
            );
            command_buffer.draw(0..3, 0..1);
            command_buffer.end_render_pass();
            command_buffer.finish();

            let queues = Pin::get_unchecked_mut(self.queues.as_mut());

            queues.graphics.as_mut().queues[0].submit(
                iter::once(&*command_buffer),
                iter::empty(),
                iter::once(&self.submit_semaphores[frame]),
                Some(fence),
            );

            let result = queues.present.as_mut().queues[0].present(
                &mut self.surface,
                surface_image,
                Some(&mut self.submit_semaphores[frame]),
            );

            if result.is_err() {
                self.recreate_swapchain();
            }
        }
    }

    pub fn recreate_swapchain(&mut self) {
        let caps = self.surface.capabilities(&self.adapter.physical_device);
        let config = SwapchainConfig::from_caps(&caps, self.format, self.dimensions);
        let framebuffer_attachment = config.framebuffer_attachment();
        self.dimensions = config.extent;
        self.viewport = Viewport {
            rect: Rect {
                x: 0,
                y: 0,
                w: self.dimensions.width as i16,
                h: self.dimensions.height as i16,
            },
            depth: 0.0..1.0,
        };

        self.device.wait_idle().unwrap();

        unsafe {
            self.surface
                .configure_swapchain(&self.device, config)
                .expect("Could not configure the swapchain")
        };

        let mut framebuffer = ManuallyDrop::new(unsafe {
            self.device
                .create_framebuffer(
                    &self.render_pass,
                    iter::once(framebuffer_attachment),
                    self.dimensions.to_extent(),
                )
                .expect("Could not create a framebuffer")
        });
        swap(&mut self.framebuffer, &mut framebuffer);

        let framebuffer = ManuallyDrop::into_inner(framebuffer);
        unsafe {
            self.device.destroy_framebuffer(framebuffer);
        }
    }
}

impl<B: Backend> Drop for GearsRenderer<B> {
    fn drop(&mut self) {
        debug!("Renderer dropped");
        self.device.wait_idle().unwrap();
        unsafe {
            for command_pool in self.command_pools.drain(..) {
                self.device.destroy_command_pool(command_pool);
            }

            for submit_fence in self.submit_fences.drain(..) {
                self.device.destroy_fence(submit_fence);
            }

            for submit_semaphore in self.submit_semaphores.drain(..) {
                self.device.destroy_semaphore(submit_semaphore);
            }

            let vertex_buffer = ManuallyDrop::into_inner(ptr::read(&self.vertex_buffer));
            vertex_buffer.destroy(&self.device);

            let pipeline = ManuallyDrop::into_inner(ptr::read(&self.pipeline));
            self.device.destroy_graphics_pipeline(pipeline);

            let framebuffer = ManuallyDrop::into_inner(ptr::read(&self.framebuffer));
            self.device.destroy_framebuffer(framebuffer);

            let render_pass = ManuallyDrop::into_inner(ptr::read(&self.render_pass));
            self.device.destroy_render_pass(render_pass);

            self.surface.unconfigure_swapchain(&self.device);

            let surface = ManuallyDrop::into_inner(ptr::read(&self.surface));
            self.instance.destroy_surface(surface);
        }
    }
}
