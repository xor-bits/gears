mod pipeline;
pub mod queue;

use std::{
    borrow::Borrow,
    iter,
    mem::{swap, ManuallyDrop},
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
    format::{ChannelType, Format},
    image::Layout,
    pass::{Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, SubpassDesc},
    pool::{CommandPool, CommandPoolCreateFlags},
    prelude::{CommandQueue, PhysicalDevice},
    pso::{Rect, Viewport},
    window::{AcquireError, Extent2D, PresentationSurface, Surface, SwapchainConfig},
    Backend, Features, Instance,
};
use pipeline::create_pipeline;

use crate::log::LogWrap;

use self::queue::{QueueFamilies, Queues};

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

    pipeline: ManuallyDrop<B::GraphicsPipeline>,

    render_pass: ManuallyDrop<B::RenderPass>,
    framebuffer: ManuallyDrop<B::Framebuffer>,
    surface: ManuallyDrop<B::Surface>,

    queues: Pin<Box<Queues<B>>>,
    device: B::Device,
    adapter: Adapter<B>,
    instance: B::Instance,

    format: Format,
    pub dimensions: Extent2D,
    viewport: Viewport,
    frame: usize,
    frames_in_flight: usize,
}

impl<B: Backend> GearsRenderer<B> {
    pub fn new(
        instance: B::Instance,
        mut surface: B::Surface,
        adapter: Adapter<B>,
        queue_families: QueueFamilies,
        extent: Extent2D,
    ) -> Self {
        log_debug!("Renderer created");

        // device

        let physical_device = &adapter.physical_device;
        let sparsely_bound = physical_device
            .features()
            .contains(Features::SPARSE_BINDING | Features::SPARSE_RESIDENCY_IMAGE_2D);
        let gpu = unsafe {
            physical_device.open(
                &queue_families.get_vec(&adapter).unwrap_log()[..],
                if sparsely_bound {
                    Features::SPARSE_BINDING | Features::SPARSE_RESIDENCY_IMAGE_2D
                } else {
                    Features::empty()
                },
            )
        }
        .unwrap();

        let queues = queue_families.get_queues(gpu.queue_groups).unwrap_log();
        let device = gpu.device;

        // swapchain

        let caps = surface.capabilities(&adapter.physical_device);
        let format =
            surface
                .supported_formats(physical_device)
                .map_or(Format::Rgba8Srgb, |formats| {
                    formats
                        .iter()
                        .find(|format| format.base_format().1 == ChannelType::Srgb)
                        .cloned()
                        .unwrap_or(Format::Rgba8Srgb)
                });
        let config = SwapchainConfig::from_caps(&caps, format, extent);
        let framebuffer_attachment = config.framebuffer_attachment();
        let extent = extent;
        unsafe {
            surface
                .configure_swapchain(&device, config)
                .expect_log("Could not configure the swapchain")
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
                .expect_log("Could not create a render pass"),
            )
        };

        let framebuffer = ManuallyDrop::new(unsafe {
            device
                .create_framebuffer(
                    &render_pass,
                    iter::once(framebuffer_attachment),
                    extent.to_extent(),
                )
                .expect_log("Could not create a framebuffer")
        });

        // graphics pipeline
        let pipeline = ManuallyDrop::new(create_pipeline::<B>(&device, &render_pass));

        // command pool for every 'frame in flight'
        let frames_in_flight = 2;
        let submit_semaphores = (0..frames_in_flight)
            .map(|_| {
                device
                    .create_semaphore()
                    .expect_log("Could not create a semaphore")
            })
            .collect::<Vec<_>>();
        let submit_fences = (0..frames_in_flight)
            .map(|_| {
                device
                    .create_fence(true)
                    .expect_log("Could not create a fence")
            })
            .collect::<Vec<_>>();
        let mut command_pools = (0..frames_in_flight)
            .map(|_| unsafe {
                device
                    .create_command_pool(
                        queues.graphics.as_ref().family,
                        CommandPoolCreateFlags::empty(),
                    )
                    .expect_log("Could not create a command pool")
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

            pipeline,

            render_pass,
            framebuffer,
            surface: ManuallyDrop::new(surface),

            queues,
            device,
            adapter,
            instance,

            format,
            dimensions: extent,
            viewport,
            frame: 0,
            frames_in_flight,
        }
    }

    pub fn render(&mut self) {
        // acquire the next image from the swapchain
        let surface_image = unsafe {
            match self.surface.acquire_image(1_000_000) {
                Ok((image, _)) => image,
                Err(AcquireError::NotReady { .. }) => {
                    // log_debug!("Frame timeout");
                    return;
                }
                Err(AcquireError::SurfaceLost(_)) => {
                    log_error!("Swapchain surface was lost (display disconnected?)");
                }
                Err(AcquireError::DeviceLost(_)) => {
                    log_error!("Device was lost (GPU disconnected?)");
                }
                Err(_) => {
                    self.recreate_swapchain();
                    return;
                }
            }
        };

        // Compute index into our resource ring buffers based on the frame number
        // and number of frames in flight. Pay close attention to where this index is needed
        // versus when the swapchain image index we got from acquire_image is needed.
        let frame_idx = self.frame as usize % self.frames_in_flight;
        // log_debug!("Render frame: {}", self.frame);
        self.frame += 1;

        // Wait for the fence of the previous submission of this frame and reset it; ensures we are
        // submitting only up to maximum number of frames_in_flight if we are submitting faster than
        // the gpu can keep up with. This would also guarantee that any resources which need to be
        // updated with a CPU->GPU data copy are not in use by the GPU, so we can perform those updates.
        // In this case there are none to be done, however.
        unsafe {
            let fence = &mut self.submit_fences[frame_idx];
            self.device
                .wait_for_fence(fence, !0)
                .expect_log("Failed to wait for fence");
            self.device
                .reset_fence(fence)
                .expect_log("Failed to reset fence");
            self.command_pools[frame_idx].reset(false);
        }

        // Rendering
        let cmd_buffer = &mut self.command_buffers[frame_idx];
        unsafe {
            cmd_buffer.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

            cmd_buffer.set_viewports(0, iter::once(self.viewport.clone()));
            cmd_buffer.set_scissors(0, iter::once(self.viewport.rect));
            cmd_buffer.bind_graphics_pipeline(&self.pipeline);

            cmd_buffer.begin_render_pass(
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
            cmd_buffer.draw(0..3, 0..1);
            cmd_buffer.end_render_pass();
            cmd_buffer.finish();

            let queues = Pin::get_unchecked_mut(self.queues.as_mut());

            queues.graphics.as_mut().queues[0].submit(
                iter::once(&*cmd_buffer),
                iter::empty(),
                iter::once(&self.submit_semaphores[frame_idx]),
                Some(&mut self.submit_fences[frame_idx]),
            );

            let result = queues.present.as_mut().queues[0].present(
                &mut self.surface,
                surface_image,
                Some(&mut self.submit_semaphores[frame_idx]),
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
        unsafe {
            self.surface
                .configure_swapchain(&self.device, config)
                .expect_log("Could not configure the swapchain")
        };
        self.viewport = Viewport {
            rect: Rect {
                x: 0,
                y: 0,
                w: self.dimensions.width as i16,
                h: self.dimensions.height as i16,
            },
            depth: 0.0..1.0,
        };

        let mut framebuffer = ManuallyDrop::new(unsafe {
            self.device
                .create_framebuffer(
                    &self.render_pass,
                    iter::once(framebuffer_attachment),
                    self.dimensions.to_extent(),
                )
                .expect_log("Could not create a framebuffer")
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
        log_debug!("Renderer dropped");
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

            let pipeline = ManuallyDrop::into_inner(ptr::read(&self.pipeline));
            self.device.destroy_graphics_pipeline(pipeline);

            let framebuffer = ManuallyDrop::into_inner(ptr::read(&self.framebuffer));
            self.device.destroy_framebuffer(framebuffer);

            let render_pass = ManuallyDrop::into_inner(ptr::read(&self.render_pass));
            self.device.destroy_render_pass(render_pass);

            let surface = ManuallyDrop::into_inner(ptr::read(&self.surface));
            self.instance.destroy_surface(surface);
        }
    }
}
