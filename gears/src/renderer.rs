pub mod buffer;
pub mod object;
pub mod pipeline;
pub mod queue;

use instant::{Duration, Instant};
use log::*;

use std::{
    borrow::{Borrow, BorrowMut},
    cell::RefCell,
    iter,
    mem::{swap, ManuallyDrop},
    ops::Deref,
    pin::Pin,
    ptr,
    rc::Rc,
    sync::Arc,
};

use gfx_hal::{
    adapter::{Adapter, MemoryType},
    command::{
        ClearColor, ClearValue, CommandBuffer, CommandBufferFlags, Level, RenderAttachmentInfo,
        SubpassContents,
    },
    device::Device,
    format::{ChannelType, Format, SurfaceType},
    image::{FramebufferAttachment, Layout},
    pass::{Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, SubpassDesc},
    pool::{CommandPool, CommandPoolCreateFlags},
    prelude::{CommandQueue, PhysicalDevice},
    pso::{Rect, Viewport},
    window::{AcquireError, Extent2D, PresentMode, PresentationSurface, Surface, SwapchainConfig},
    Backend, Features, Instance,
};

use buffer::Image;
use queue::{QueueFamilies, Queues};

pub type Handle<T> = Rc<RefCell<ManuallyDrop<T>>>;
pub type FrameCommands<B> = <B as Backend>::CommandBuffer;

pub struct FrameInfo<'a, B: Backend> {
    pub width: u32,
    pub height: u32,
    pub aspect: f32,
    pub delta_time: Duration,

    pub commands: &'a mut FrameCommands<B>,
    pub frame_in_flight: usize,
}

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

    // vertex_buffers: Vec<Handle<VertexBuffer<B>>>,

    // index_buffer: ManuallyDrop<IndexBuffer<B>>,
    /* vertex_buffer: ManuallyDrop<VertexBuffer<B>>,
    pipeline: ManuallyDrop<Pipeline<B>>, */
    render_pass: ManuallyDrop<B::RenderPass>,
    depth_image: ManuallyDrop<Image<B>>,
    framebuffer: ManuallyDrop<B::Framebuffer>,
    surface: ManuallyDrop<B::Surface>,

    queues: Pin<Box<Queues<B>>>,
    device: Arc<B::Device>,
    adapter: Adapter<B>,
    instance: B::Instance,

    memory_types: Vec<MemoryType>,
    format: Format,
    pub dimensions: Extent2D,
    viewport: Viewport,
    vsync: bool,
    frame: usize,
    frames_in_flight: usize,
    frametime_array: [Duration; 1000],
    frame_counter_tp: Instant,
}

impl<B: Backend> GearsRenderer<B> {
    pub fn new(
        instance: B::Instance,
        mut surface: B::Surface,
        adapter: Adapter<B>,
        queue_families: QueueFamilies,
        extent: Extent2D,
        vsync: bool,
    ) -> Self {
        debug!("Renderer created");

        // device

        let physical_device = &adapter.physical_device;
        let sparsely_bound = physical_device
            .features()
            .contains(Features::SPARSE_BINDING | Features::SPARSE_RESIDENCY_IMAGE_2D);
        let geometry_shader = physical_device
            .features()
            .contains(Features::GEOMETRY_SHADER);
        let non_fill_polygon_mode = physical_device
            .features()
            .contains(Features::NON_FILL_POLYGON_MODE);
        debug!(
            "sparsely_bound: {}, geometry_shader: {}, non_fill_polygon_mode: {}",
            sparsely_bound, geometry_shader, non_fill_polygon_mode
        );
        let gpu = unsafe {
            physical_device.open(
                &queue_families.get_vec(&adapter).unwrap()[..],
                if sparsely_bound {
                    Features::SPARSE_BINDING | Features::SPARSE_RESIDENCY_IMAGE_2D
                } else {
                    Features::empty()
                } | if geometry_shader {
                    Features::GEOMETRY_SHADER
                } else {
                    Features::empty()
                } | if non_fill_polygon_mode {
                    Features::NON_FILL_POLYGON_MODE
                } else {
                    Features::empty()
                },
            )
        }
        .unwrap();

        let queues = queue_families.get_queues(gpu.queue_groups).unwrap();
        let device = Arc::new(gpu.device);

        // swapchain
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
        let config = swap_config::<B>(&surface, &physical_device, format, extent, vsync);
        let color_fat = config.framebuffer_attachment();
        let extent = extent;
        unsafe {
            surface
                .configure_swapchain(device.deref(), config)
                .expect("Could not configure the swapchain")
        };

        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let depth_image = ManuallyDrop::new(
            Image::new_depth_texture_with_device(
                device.clone(),
                &memory_types,
                extent.width,
                extent.height,
            )
            .unwrap(),
        );
        let depth_fat = depth_image.framebuffer_attachment();

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
            let color_attachment = Attachment {
                format: Some(format),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::Store),
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::Undefined..Layout::Present,
            };

            let depth_attachment = Attachment {
                format: Some(Format::D32Sfloat),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::DontCare),
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::Undefined..Layout::DepthStencilAttachmentOptimal,
            };

            let subpass = SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: Some(&(1, Layout::DepthStencilAttachmentOptimal)),
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            ManuallyDrop::new(
                unsafe {
                    device.create_render_pass(
                        [color_attachment, depth_attachment].iter().cloned(),
                        std::iter::once(subpass),
                        std::iter::empty(),
                    )
                }
                .expect("Could not create a render pass"),
            )
        };

        let framebuffer =
            create_framebuffer::<B>(&device, &render_pass, extent, color_fat, depth_fat);

        // graphics pipeline
        let frames_in_flight = 3;
        debug!("memory_types: {:?}", memory_types);
        /* let pipeline = ManuallyDrop::new(
            PipelineBuilder::new(&device, &*render_pass, &memory_types, frames_in_flight)
                .with_input::<shader::VertexData>()
                .with_module_vert(shader::VERT_SPIRV)
                .with_module_frag(shader::FRAG_SPIRV)
                .with_ubo::<shader::UBO>()
                .build(),
        );

        // create vertex&index buffer
        let vertices = load_obj(
            include_str!("../res/gears_smooth.obj"),
            None,
            |position, normal| shader::VertexData {
                pos: position,
                norm: normal,
            },
        );
        let vertex_buffer =
            ManuallyDrop::new(VertexBuffer::new_with(&device, &memory_types, &vertices)); */

        // command pool for every 'frame in flight'
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

            // vertex_buffers: Vec::new(),
            /* vertex_buffer,
            pipeline, */
            render_pass,
            depth_image,
            framebuffer,
            surface: ManuallyDrop::new(surface),

            queues,
            device,
            adapter,
            instance,

            memory_types,
            format,
            dimensions: extent,
            viewport,
            vsync,
            frame: 0,
            frames_in_flight,
            frametime_array: [Duration::from_nanos(0); 1000],
            frame_counter_tp: Instant::now(),
        }
    }

    pub fn begin_render(
        &mut self,
    ) -> Option<(
        FrameInfo<B>,
        <<B as gfx_hal::Backend>::Surface as PresentationSurface<B>>::SwapchainImage,
        usize,
        usize,
        Instant,
    )> {
        // acquire the next image from the swapchain
        let swapchain_image = match unsafe { self.surface.acquire_image(!0) } {
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
                return None;
            }
        };

        let frame = self.frame % self.frames_in_flight;
        let frametime_id = self.frame % self.frametime_array.len();
        let frametime_tp = Instant::now();
        self.frame += 1;

        unsafe {
            let fence = &mut self.submit_fences[frame];
            self.device
                .wait_for_fence(fence, !0)
                .expect("Failed to wait for fence");
            self.device
                .reset_fence(fence)
                .expect("Failed to reset fence");
            self.command_pools[frame].reset(false);
        };

        // print average fps every 3 seconds
        let avg_fps_interval = Duration::from_secs_f32(3.0);
        if self.frame_counter_tp.elapsed() > avg_fps_interval {
            self.frame_counter_tp = Instant::now();
            let avg = self
                .frametime_array
                .iter()
                .sum::<Duration>()
                .div_f32(self.frametime_array.len() as f32 / self.frames_in_flight as f32);

            debug!(
                "Average frametime: {:?} ms (~{} fps)",
                avg.as_micros() as f32 / 1000.0,
                1.0 / avg.as_secs_f32()
            );
            /* debug!("Triangles: {}", self.vertex_buffer.count() / 3); */
        }

        // Rendering
        let command_buffer = unsafe {
            // begin recording
            let command_buffer = self.command_buffers[frame].borrow_mut();
            command_buffer.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

            let iter = iter::once(RenderAttachmentInfo {
                image_view: swapchain_image.borrow(),
                clear_value: ClearValue {
                    color: ClearColor {
                        float32: [0.18, 0.18, 0.2, 1.0],
                    },
                },
            })
            .chain(iter::once(RenderAttachmentInfo {
                image_view: self.depth_image.view(),
                clear_value: ClearValue {
                    color: ClearColor {
                        float32: [1.0, 1.0, 1.0, 1.0],
                    },
                },
            }));

            // begin render pass
            command_buffer.set_viewports(0, iter::once(self.viewport.clone()));
            command_buffer.set_scissors(0, iter::once(self.viewport.rect));
            command_buffer.begin_render_pass(
                &self.render_pass,
                &self.framebuffer,
                self.viewport.rect,
                iter,
                SubpassContents::Inline,
            );
            command_buffer
        };

        // main draw
        let aspect = {
            let mut aspect = (self.dimensions.width as f32) / (self.dimensions.height as f32);
            if aspect.is_nan() {
                aspect = 1.0
            };
            aspect
        };
        Some((
            FrameInfo {
                width: self.dimensions.width,
                height: self.dimensions.height,
                aspect,
                delta_time: self.frametime_array
                    [(self.frame + self.frametime_array.len() - 2) % self.frametime_array.len()],
                commands: command_buffer,
                frame_in_flight: frame,
            },
            swapchain_image,
            frame,
            frametime_id,
            frametime_tp,
        ))
    }

    pub fn end_render(
        &mut self,
        swapchain_image: <<B as gfx_hal::Backend>::Surface as PresentationSurface<B>>::SwapchainImage,
        frame: usize,
        frametime_id: usize,
        frametime_tp: Instant,
    ) {
        unsafe {
            let command_buffer = self.command_buffers[frame].borrow_mut();
            // stop render pass
            command_buffer.end_render_pass();

            // stop recording
            command_buffer.finish();

            // submit
            let queues = Pin::get_unchecked_mut(self.queues.as_mut());
            queues.graphics.as_mut().queues[0].submit(
                iter::once(&*command_buffer),
                iter::empty(),
                iter::once(&self.submit_semaphores[frame]),
                Some(&mut self.submit_fences[frame]),
            );

            // present
            let result = queues.present.as_mut().queues[0].present(
                &mut self.surface,
                swapchain_image,
                Some(&mut self.submit_semaphores[frame]),
            );

            // recreate swapchain if needed
            if result.is_err() {
                self.recreate_swapchain();
            }
        }

        self.frametime_array[frametime_id] = frametime_tp.elapsed();
    }

    pub fn recreate_swapchain(&mut self) {
        let config = swap_config::<B>(
            &self.surface,
            &self.adapter.physical_device,
            self.format,
            self.dimensions,
            self.vsync,
        );

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

        let mut depth_image = ManuallyDrop::new(
            Image::new_depth_texture_with_device(
                self.device.clone(),
                &self.memory_types,
                self.dimensions.width,
                self.dimensions.height,
            )
            .unwrap(),
        );
        swap(&mut self.depth_image, &mut depth_image);

        let depth_image = ManuallyDrop::into_inner(depth_image);
        drop(depth_image);

        let mut framebuffer = create_framebuffer::<B>(
            &self.device,
            &self.render_pass,
            self.dimensions,
            framebuffer_attachment,
            self.depth_image.framebuffer_attachment(),
        );
        swap(&mut self.framebuffer, &mut framebuffer);

        let framebuffer = ManuallyDrop::into_inner(framebuffer);
        unsafe {
            self.device.destroy_framebuffer(framebuffer);
        }
    }

    pub fn wait(&self) {
        self.device.wait_idle().unwrap();
    }
}

impl<B: Backend> Drop for GearsRenderer<B> {
    fn drop(&mut self) {
        self.wait();

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

            /* for vertex_buffer in self.vertex_buffers.drain(..) {
                let vertex_buffer = ManuallyDrop::into_inner(ptr::read(vertex_buffer.as_ptr()));
                vertex_buffer.destroy(&self.device);
            } */

            /* let vertex_buffer = ManuallyDrop::into_inner(ptr::read(&self.vertex_buffer));
            vertex_buffer.destroy(&self.device);

            let pipeline = ManuallyDrop::into_inner(ptr::read(&self.pipeline));
            pipeline.destroy(&self.device); */

            let depth_image = ManuallyDrop::into_inner(ptr::read(&self.depth_image));
            drop(depth_image);
            let framebuffer = ManuallyDrop::into_inner(ptr::read(&self.framebuffer));
            self.device.destroy_framebuffer(framebuffer);

            let render_pass = ManuallyDrop::into_inner(ptr::read(&self.render_pass));
            self.device.destroy_render_pass(render_pass);

            self.surface.unconfigure_swapchain(&self.device);

            let surface = ManuallyDrop::into_inner(ptr::read(&self.surface));
            self.instance.destroy_surface(surface);
        }
        debug!("Renderer dropped");
    }
}

fn create_framebuffer<B: Backend>(
    device: &B::Device,
    render_pass: &B::RenderPass,
    extent: Extent2D,
    color_fat: FramebufferAttachment,
    depth_fat: FramebufferAttachment,
) -> ManuallyDrop<B::Framebuffer> {
    ManuallyDrop::new(unsafe {
        device
            .create_framebuffer(
                &render_pass,
                [color_fat, depth_fat].iter().cloned(),
                extent.to_extent(),
            )
            .expect("Could not create a framebuffer")
    })
}

fn swap_config<B: Backend>(
    surface: &B::Surface,
    physical_device: &B::PhysicalDevice,
    format: Format,
    extent: Extent2D,
    vsync: bool,
) -> SwapchainConfig {
    let caps = surface.capabilities(physical_device);
    debug!("Present modes available: {:?}", caps.present_modes);
    let present_mode = if !vsync {
        if caps.present_modes.contains(PresentMode::IMMEDIATE) {
            PresentMode::IMMEDIATE
        } else if caps.present_modes.contains(PresentMode::MAILBOX) {
            PresentMode::MAILBOX
        } else if caps.present_modes.contains(PresentMode::FIFO) {
            PresentMode::FIFO
        } else {
            panic!("MAILBOX, FIFO nor IMMEDIATE PresentMode is not supported")
        }
    } else {
        if caps.present_modes.contains(PresentMode::MAILBOX) {
            PresentMode::MAILBOX
        } else if caps.present_modes.contains(PresentMode::FIFO) {
            PresentMode::FIFO
        } else if caps.present_modes.contains(PresentMode::IMMEDIATE) {
            PresentMode::IMMEDIATE
        } else {
            panic!("MAILBOX, FIFO nor IMMEDIATE PresentMode is not supported")
        }
    };

    SwapchainConfig::from_caps(&caps, format, extent).with_present_mode(present_mode)
}
