mod shader {
    gears_pipeline::pipeline! {
        vs: { path: "res/default.vert.glsl" }
        fs: { path: "res/default.frag.glsl" }
    }
}

mod buffer;
mod pipeline;
pub mod queue;

use cgmath::*;
use log::*;
use wavefront_obj::obj::Primitive;

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
    format::{ChannelType, Format, SurfaceType},
    image::{FramebufferAttachment, Layout},
    pass::{Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, SubpassDesc},
    pool::{CommandPool, CommandPoolCreateFlags},
    prelude::{CommandQueue, PhysicalDevice},
    pso::{Rect, Viewport},
    window::{AcquireError, Extent2D, PresentMode, PresentationSurface, Surface, SwapchainConfig},
    Backend, Features, Instance,
};

use pipeline::{Pipeline, PipelineBuilder};

use self::{
    buffer::{Buffer, Image, VertexBuffer},
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

    // index_buffer: ManuallyDrop<IndexBuffer<B>>,
    vertex_buffer: ManuallyDrop<VertexBuffer<B>>,
    pipeline: ManuallyDrop<Pipeline<B>>,

    render_pass: ManuallyDrop<B::RenderPass>,
    depth_image: ManuallyDrop<Image<B>>,
    framebuffer: ManuallyDrop<B::Framebuffer>,
    surface: ManuallyDrop<B::Surface>,

    queues: Pin<Box<Queues<B>>>,
    device: B::Device,
    adapter: Adapter<B>,
    instance: B::Instance,

    format: Format,
    pub dimensions: Extent2D,
    viewport: Viewport,
    vsync: bool,
    frame: usize,
    frames_in_flight: usize,
    frame_counter: usize,
    frame_counter_tp: instant::Instant,
    start_tp: instant::Instant,
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
                .configure_swapchain(&device, config)
                .expect("Could not configure the swapchain")
        };

        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let depth_image = ManuallyDrop::new(Image::new_depth_texture(
            &device,
            &memory_types,
            extent.width,
            extent.height,
        ));
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
        let pipeline =
            PipelineBuilder::new(&device, &*render_pass, &memory_types, frames_in_flight)
                .with_input::<shader::VertexData>()
                .with_module_vert(shader::VERT_SPIRV)
                .with_module_frag(shader::FRAG_SPIRV)
                .with_ubo::<shader::UBO>()
                .build();
        let pipeline = ManuallyDrop::new(pipeline);

        // create vertex&index buffer
        let objset = wavefront_obj::obj::parse(include_str!("../res/gears.obj")).unwrap();
        // let mtl = wavefront_obj::mtl::parse(include_str!("../res/gears.mtl")).unwrap();
        let obj = &objset.objects[0];
        let i_count = obj
            .geometry
            .iter()
            .map(|g| {
                g.shapes
                    .iter()
                    .map(|s| match &s.primitive {
                        Primitive::Triangle(_, _, _) => 3,
                        _ => panic!("Only triangles"),
                    })
                    .sum::<usize>()
            })
            .sum::<usize>();

        /* let mut index_buffer = ManuallyDrop::new(IndexBuffer::new(&device, &memory_types, i_count));
        let mut indices = Vec::<u32>::with_capacity(i_count);
        for g in obj.geometry.iter() {
            for s in g.shapes.iter() {
                match &s.primitive {
                    Primitive::Triangle(a, b, c) => {
                        indices.extend([a.0 as u32, b.0 as u32, c.0 as u32].iter());
                    }
                    _ => panic!("Only triangles"),
                }
            }
        }
        index_buffer.write(&device, 0, &indices); */
        let mut vertex_buffer = ManuallyDrop::new(VertexBuffer::new::<shader::VertexData>(
            &device,
            &memory_types,
            i_count,
        ));

        // fill vertex&index buffer
        let mut vertices = Vec::<shader::VertexData>::with_capacity(i_count);
        for g in obj.geometry.iter() {
            for s in g.shapes.iter() {
                match s.primitive {
                    Primitive::Triangle(
                        (a_vert_id, _, a_norm_id),
                        (b_vert_id, _, b_norm_id),
                        (c_vert_id, _, c_norm_id),
                    ) => {
                        let id_to_vertex = |vert: usize, norm: Option<usize>| {
                            let vert = obj.vertices[vert];
                            let norm = obj.normals[norm.unwrap()];

                            shader::VertexData {
                                pos: Vector3::new(vert.x as f32, vert.y as f32, vert.z as f32),
                                norm: Vector3::new(norm.x as f32, norm.y as f32, norm.z as f32), /* Vector3::new(norm.x as f32, norm.y as f32, norm.z as f32), */
                            }
                        };

                        vertices.extend_from_slice(&[
                            id_to_vertex(a_vert_id, a_norm_id),
                            id_to_vertex(b_vert_id, b_norm_id),
                            id_to_vertex(c_vert_id, c_norm_id),
                        ]);
                    }
                    _ => panic!("Only triangles"),
                }
            }
        }
        vertex_buffer.write(&device, 0, &vertices);

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

            vertex_buffer,
            pipeline,

            render_pass,
            depth_image,
            framebuffer,
            surface: ManuallyDrop::new(surface),

            queues,
            device,
            adapter,
            instance,

            format,
            dimensions: extent,
            viewport,
            vsync,
            frame: 0,
            frames_in_flight,
            frame_counter: 0,
            frame_counter_tp: instant::Instant::now(),
            start_tp: instant::Instant::now(),
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

        let t = self.start_tp.elapsed().as_secs_f32();
        let ubo = shader::UBO {
            model_matrix: Matrix4::from_scale(1.0),
            view_matrix: Matrix4::look_at_rh(
                Point3::new(t.sin() * 3.0, (t * 0.2).sin() * 3.0, t.cos() * 3.0),
                Point3::new(0.0, 0.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
            ),
            projection_matrix: perspective(Deg { 0: 60.0 }, 1.0, 0.01, 100.0),
            light_dir: Vector3::new(1.0, 2.0, 0.5).normalize(),
        };
        // debug!("ubo = {:#?}", ubo);
        self.pipeline.write_ubo(&self.device, ubo, frame);

        // print average fps every 3 seconds
        let avg_fps_interval = instant::Duration::from_secs_f32(3.0);
        if self.frame_counter_tp.elapsed() > avg_fps_interval {
            self.frame_counter /= self.frames_in_flight;
            let time_per_frame = avg_fps_interval
                .checked_div(self.frame_counter as u32)
                .unwrap_or(instant::Duration::from_secs_f64(0.0));
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
            // begin recording
            let command_buffer = &mut self.command_buffers[frame];
            command_buffer.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

            // slice doesn't work for some odd reason
            let iter = vec![
                RenderAttachmentInfo {
                    image_view: surface_image.borrow(),
                    clear_value: ClearValue {
                        color: ClearColor {
                            float32: [0.18, 0.18, 0.2, 1.0],
                        },
                    },
                },
                RenderAttachmentInfo {
                    image_view: self.depth_image.view(),
                    clear_value: ClearValue {
                        color: ClearColor {
                            float32: [1.0, 1.0, 1.0, 1.0],
                        },
                    },
                },
            ]
            .into_iter();

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

            // main draw
            self.pipeline.bind(command_buffer, frame);
            self.vertex_buffer.bind(command_buffer);
            /* self.index_buffer.bind(command_buffer);
            command_buffer.draw_indexed(0..(self.index_buffer.count() as u32), 0, 0..1); */
            command_buffer.draw(0..(self.vertex_buffer.count() as u32), 0..1);

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
                Some(fence),
            );

            // present
            let result = queues.present.as_mut().queues[0].present(
                &mut self.surface,
                surface_image,
                Some(&mut self.submit_semaphores[frame]),
            );

            // recreate swapchain if needed
            if result.is_err() {
                self.recreate_swapchain();
            }
        }
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

            /* let index_buffer = ManuallyDrop::into_inner(ptr::read(&self.index_buffer));
            index_buffer.destroy(&self.device); */
            let vertex_buffer = ManuallyDrop::into_inner(ptr::read(&self.vertex_buffer));
            vertex_buffer.destroy(&self.device);

            let pipeline = ManuallyDrop::into_inner(ptr::read(&self.pipeline));
            pipeline.destroy(&self.device);

            let depth_image = ManuallyDrop::into_inner(ptr::read(&self.depth_image));
            depth_image.destroy(&self.device);
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
