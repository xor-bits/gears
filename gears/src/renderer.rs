pub mod buffer;
mod device;
pub mod object;
pub mod pipeline;
pub mod query;
pub mod queue;

#[cfg(feature = "short_namespaces")]
pub use buffer::*;
use cgmath::Vector4;
#[cfg(feature = "short_namespaces")]
pub use object::*;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
#[cfg(feature = "short_namespaces")]
pub use pipeline::*;
#[cfg(feature = "short_namespaces")]
pub use query::*;
#[cfg(feature = "short_namespaces")]
pub use queue::*;

use crate::{
    context::{Context, ContextError},
    renderer::device::ReducedContext,
    MapErrorElseLogResult, MapErrorLog, SyncMode,
};

use ash::{extensions::khr, version::DeviceV1_0, vk};
use buffer::{image::Image, image::ImageBuilder, image::ImageFormat, image::ImageUsage};
use log::{debug, error};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use self::{
    buffer::image::BaseFormat,
    device::RenderDevice,
    query::{PerfQuery, PerfQueryResult},
};

pub struct FramePerfReport {
    pub cpu_frametime: Duration,
    pub gpu_frametime: PerfQueryResult,

    pub rerecord: bool,
    pub updates: bool,
    pub triangles: usize,
}

struct SwapchainObjects {
    extent: vk::Extent2D,
    format: vk::SurfaceFormatKHR,
    present: vk::PresentModeKHR,
    viewport: vk::Viewport,
    scissor: vk::Rect2D,

    render_pass: vk::RenderPass,

    swapchain_loader: khr::Swapchain,
    swapchain: vk::SwapchainKHR,

    surface_loader: khr::Surface,
    surface: vk::SurfaceKHR,
}

struct RenderObject {
    rerecord_requested: bool,
    image_in_use_fence: vk::Fence,

    _color_image: Image,
    _depth_image: Image,
    framebuffer: vk::Framebuffer,

    render_cb: vk::CommandBuffer,
    update_cb: vk::CommandBuffer,
    update_cb_recording: bool,
    update_cb_pending: bool,
    command_pool: vk::CommandPool,
    perf: PerfQuery,
    triangles: usize,
}

struct ConcurrentRenderObject {
    frame_fence: vk::Fence,
    image_semaphore: vk::Semaphore,
    update_semaphore: vk::Semaphore,
    render_semaphore: vk::Semaphore,
}

/* enum PresentThreadEvent {
    // 0 = frame, 1 = image_index
    // todo: typesafe this
    PresentImage(u32, u32),
    Stop,
} */

pub struct ImmediateFrameInfo {
    pub image_index: usize,
}

pub struct RenderRecordInfo {
    command_buffer: vk::CommandBuffer,
    image_index: usize,
    triangles: AtomicUsize,
    debug_calls: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RenderRecordBeginInfo {
    pub debug_calls: bool,
    pub clear_color: Vector4<f32>,
}

pub struct UpdateRecordInfo {
    command_buffer: vk::CommandBuffer,
    image_index: usize,
}

pub trait RendererRecord {
    #[allow(unused_variables)]
    fn immediate(&self, imfi: &ImmediateFrameInfo) {}

    #[allow(unused_variables)]
    fn update(&self, uri: &UpdateRecordInfo) -> bool {
        // 'any' all object updates and return the result of that
        false
    }

    #[allow(unused_variables)]
    fn begin_info(&self) -> RenderRecordBeginInfo {
        RenderRecordBeginInfo {
            clear_color: Vector4::new(0.18, 0.18, 0.2, 1.0),
            debug_calls: false,
        }
    }

    #[allow(unused_variables)]
    fn record(&self, rri: &RenderRecordInfo) {}
}

pub struct RendererData {
    swapchain_objects: RwLock<SwapchainObjects>,
    render_objects: Vec<RwLock<RenderObject>>,
    crender_objects: Vec<RwLock<ConcurrentRenderObject>>,
}

pub struct Renderer {
    /* present_thread_join: Option<JoinHandle<()>>,
    main_thread_tx: Mutex<mpsc::Sender<PresentThreadEvent>>,
    main_thread_rx: Mutex<mpsc::Receiver<bool>>, */
    data: Arc<RwLock<RendererData>>,

    frame: AtomicUsize,
    frames_in_flight: usize,

    rdevice: Arc<RenderDevice>,
}

pub struct RendererBuilder {
    sync: SyncMode,
    frames_in_flight: usize,
}

impl Default for FramePerfReport {
    fn default() -> Self {
        Self {
            cpu_frametime: Duration::from_secs(0),
            gpu_frametime: PerfQueryResult::default(),

            rerecord: false,
            updates: false,

            triangles: 0,
        }
    }
}

impl RenderObject {
    fn new(
        rdevice: Arc<RenderDevice>,
        render_pass: vk::RenderPass,
        color_image: vk::Image,
        color_format: vk::Format,
        extent: vk::Extent2D,
    ) -> Result<Self, ContextError> {
        let color_image = ImageBuilder::new_with_device(rdevice.clone())
            .build_with_image(color_image, ImageUsage::WRITE, color_format)
            .map_err_log("Color image creation failed", ContextError::OutOfMemory)?;

        let depth_image = ImageBuilder::new_with_device(rdevice.clone())
            .with_width(extent.width)
            .with_height(extent.height)
            .build(ImageUsage::WRITE, ImageFormat::<f32>::D)
            .map_err_log("Depth image creation failed", ContextError::OutOfMemory)?;

        let attachments = [color_image.view(), depth_image.view()];

        let framebuffer_info = vk::FramebufferCreateInfo::builder()
            .attachments(&attachments)
            .render_pass(render_pass)
            .width(extent.width)
            .height(extent.height)
            .layers(1);

        let framebuffer = unsafe { rdevice.create_framebuffer(&framebuffer_info, None) }
            .map_err_log("Framebuffer creation failed", ContextError::OutOfMemory)?;

        let command_pool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(rdevice.queues.graphics_family as u32)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        let command_pool = unsafe { rdevice.create_command_pool(&command_pool_info, None) }
            .map_err_log("Command pool creation failed", ContextError::OutOfMemory)?;

        let command_buffer_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(2)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(command_pool);

        let mut command_buffers = unsafe { rdevice.allocate_command_buffers(&command_buffer_info) }
            .map_err_log(
                "Command buffer allocation failed",
                ContextError::OutOfMemory,
            )?;

        if command_buffers.len() != 2 {
            unreachable!("Allocated command buffer count not 1");
        }

        let update_cb = command_buffers.remove(0);
        let render_cb = command_buffers.remove(0);

        Ok(Self {
            rerecord_requested: true,
            image_in_use_fence: vk::Fence::null(),

            _color_image: color_image,
            _depth_image: depth_image,
            framebuffer,

            render_cb,
            update_cb,
            update_cb_recording: false,
            update_cb_pending: false,
            command_pool,
            perf: PerfQuery::new_with_device(rdevice),
            triangles: 0,
        })
    }
}

impl ConcurrentRenderObject {
    fn new(device: &Arc<RenderDevice>) -> Result<Self, ContextError> {
        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

        Ok(Self {
            frame_fence: unsafe { device.create_fence(&fence_info, None) }
                .map_err_log("Fence creation failed", ContextError::OutOfMemory)?,
            image_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .map_err_log("Semaphore creation failed", ContextError::OutOfMemory)?,
            update_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .map_err_log("Semaphore creation failed", ContextError::OutOfMemory)?,
            render_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .map_err_log("Semaphore creation failed", ContextError::OutOfMemory)?,
        })
    }
}

impl Renderer {
    pub fn new() -> RendererBuilder {
        RendererBuilder {
            sync: SyncMode::default(),
            frames_in_flight: 3,
        }
    }

    fn wait_in_use_render_object(&self, crender_object: &RwLockReadGuard<ConcurrentRenderObject>) {
        unsafe {
            let fence = [crender_object.frame_fence];
            self.rdevice
                .wait_for_fences(&fence, true, !0)
                .expect("Failed to wait for fence");
        }
    }

    fn acquire_image(&self, crender_object: &RwLockReadGuard<ConcurrentRenderObject>) -> usize {
        let data = self.data.read();
        let swapchain_objects = data.swapchain_objects.read();

        match unsafe {
            swapchain_objects.swapchain_loader.acquire_next_image(
                swapchain_objects.swapchain,
                1_000_000_000_000_000,
                crender_object.image_semaphore,
                vk::Fence::null(),
            )
        } {
            Ok((image_index, _)) => image_index as usize,
            Err(err) => panic!("Failed to aquire image from swapchain: {:?}", err),
        }
    }

    pub fn frame<T: RendererRecord>(&self, recorder: &T) -> FramePerfReport {
        let cpu_frametime = Instant::now();

        // thread::sleep(Duration::from_millis(15));

        let data = self.data.read();

        // increment and get the next frame object index
        let frame = self.frame.fetch_add(1, Ordering::SeqCst) % self.frames_in_flight;
        let crender_object = data.crender_objects[frame].read();

        self.wait_in_use_render_object(&crender_object);

        // aquire image
        let image_index = self.acquire_image(&crender_object);

        let mut render_object = data.render_objects[image_index].write();
        if render_object.image_in_use_fence != vk::Fence::null() {
            unsafe {
                let fence = [render_object.image_in_use_fence];
                self.rdevice
                    .wait_for_fences(&fence, true, !0)
                    .expect("Failed to wait for fence");
            }
        }
        render_object.image_in_use_fence = crender_object.frame_fence;
        unsafe {
            let fence = [crender_object.frame_fence];
            self.rdevice
                .reset_fences(&fence)
                .expect("Failed to reset fence");
        }

        // update buffers
        self.update(recorder, &mut render_object, image_index);
        self.immediate(recorder, image_index);
        let rerecord = render_object.rerecord_requested;
        if rerecord {
            self.record(recorder, &mut render_object, image_index);
            render_object.rerecord_requested = false;
        }
        let gpu_frametime = render_object
            .perf
            .get()
            .unwrap_or(PerfQueryResult::default());

        // submit
        unsafe {
            let render_cb = [render_object.render_cb];
            let image_wait = [crender_object.image_semaphore];
            let update_wait = [crender_object.update_semaphore];
            let render_wait = [crender_object.render_semaphore];
            let render_stage = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

            let submit_render = if render_object.update_cb_pending {
                let update_cb = [render_object.update_cb];
                let update_stage = [vk::PipelineStageFlags::ALL_COMMANDS];

                let submit_update = [vk::SubmitInfo::builder()
                    .command_buffers(&update_cb)
                    .wait_semaphores(&image_wait)
                    .signal_semaphores(&update_wait)
                    .wait_dst_stage_mask(&update_stage)
                    .build()];

                self.rdevice
                    .queue_submit(
                        self.rdevice.queues.graphics,
                        &submit_update,
                        vk::Fence::null(),
                    )
                    .expect("Transfer queue submit failed");

                [vk::SubmitInfo::builder()
                    .command_buffers(&render_cb)
                    .wait_semaphores(&update_wait)
                    .signal_semaphores(&render_wait)
                    .wait_dst_stage_mask(&render_stage)
                    .build()]
            } else {
                [vk::SubmitInfo::builder()
                    .command_buffers(&render_cb)
                    .wait_semaphores(&image_wait)
                    .signal_semaphores(&render_wait)
                    .wait_dst_stage_mask(&render_stage)
                    .build()]
            };

            self.rdevice
                .queue_submit(
                    self.rdevice.queues.graphics,
                    &submit_render,
                    crender_object.frame_fence,
                )
                .expect("Graphics queue submit failed");
        }

        let updates = render_object.update_cb_pending;
        let triangles = render_object.triangles;
        drop(render_object);

        // present
        let suboptimal = {
            let swapchain_objects = data.swapchain_objects.read();
            let crender_object = data.crender_objects[frame as usize].read();
            let wait = [crender_object.render_semaphore];
            let swapchain = [swapchain_objects.swapchain];
            let image_index = [image_index as u32];

            let submit_present = vk::PresentInfoKHR::builder()
                .wait_semaphores(&wait)
                .swapchains(&swapchain)
                .image_indices(&image_index);

            unsafe {
                // present
                let result = swapchain_objects
                    .swapchain_loader
                    .queue_present(self.rdevice.queues.present, &submit_present);
                match result {
                    Ok(o) => o,
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
                    Err(e) => panic!("Present queue submit failed: {:?}", e),
                }
            }
        };

        // recreate swapchain if needed
        if suboptimal {
            self.recreate_swapchain();
        }

        /* // recreate swapchain if needed
        let suboptimal = self
            .main_thread_rx
            .lock()
            .try_recv()
            .map_or(false, |suboptimal| suboptimal);
        if suboptimal {
            self.recreate_swapchain();
        }

        // present
        self.main_thread_tx
            .lock()
            .send(PresentThreadEvent::PresentImage(
                frame as u32,
                image_index as u32,
            ))
            .unwrap(); */

        FramePerfReport {
            cpu_frametime: cpu_frametime.elapsed(),
            gpu_frametime: gpu_frametime,

            rerecord,
            updates,
            triangles,
        }
    }

    fn update<T: RendererRecord>(
        &self,
        recorder: &T,
        render_object: &mut RwLockWriteGuard<RenderObject>,
        image_index: usize,
    ) {
        if render_object.update_cb_recording == false {
            unsafe {
                self.rdevice.reset_command_buffer(
                    render_object.update_cb,
                    vk::CommandBufferResetFlags::empty(),
                )
            }
            .expect("Command buffer reset failed");
            unsafe {
                self.rdevice.begin_command_buffer(
                    render_object.update_cb,
                    &vk::CommandBufferBeginInfo::builder(),
                )
            }
            .expect("Command buffer begin failed");

            render_object.update_cb_recording = true;
        }

        let uri = UpdateRecordInfo {
            command_buffer: render_object.update_cb,
            image_index,
        };
        render_object.update_cb_pending = recorder.update(&uri);

        if render_object.update_cb_pending {
            render_object.update_cb_recording = false;
            unsafe { self.rdevice.end_command_buffer(render_object.update_cb) }
                .expect("Command buffer end failed");
        }
    }

    fn immediate<T: RendererRecord>(&self, recorder: &T, image_index: usize) {
        let imfi = ImmediateFrameInfo { image_index };
        recorder.immediate(&imfi)
    }

    fn record<T: RendererRecord>(
        &self,
        recorder: &T,
        render_object: &mut RwLockWriteGuard<RenderObject>,
        image_index: usize,
    ) {
        let begin_info = recorder.begin_info();

        let data = self.data.read();
        let swapchain_objects = data.swapchain_objects.read();
        let rri = RenderRecordInfo {
            command_buffer: render_object.render_cb,
            image_index,
            triangles: AtomicUsize::new(0),
            debug_calls: begin_info.debug_calls,
        };

        let viewport = [swapchain_objects.viewport];
        let scissor = [swapchain_objects.scissor];
        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [
                        begin_info.clear_color.x,
                        begin_info.clear_color.y,
                        begin_info.clear_color.z,
                        begin_info.clear_color.w,
                    ],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            },
        ];
        let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
            .clear_values(&clear_values)
            .framebuffer(render_object.framebuffer)
            .render_pass(swapchain_objects.render_pass)
            .render_area(swapchain_objects.scissor);

        unsafe {
            if begin_info.debug_calls {
                debug!("begin_command_buffer with: {:?}", begin_info);
            }

            self.rdevice
                .reset_command_buffer(
                    render_object.render_cb,
                    vk::CommandBufferResetFlags::empty(),
                )
                .expect("Command buffer reset failed");

            self.rdevice
                .begin_command_buffer(
                    render_object.render_cb,
                    &vk::CommandBufferBeginInfo::builder(),
                )
                .expect("Command buffer begin failed");

            render_object.perf.reset(&rri);

            self.rdevice
                .cmd_set_viewport(render_object.render_cb, 0, &viewport);

            self.rdevice
                .cmd_set_scissor(render_object.render_cb, 0, &scissor);

            drop(swapchain_objects);

            self.rdevice.cmd_begin_render_pass(
                render_object.render_cb,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );

            recorder.record(&rri);
            render_object.perf.bind(&rri);
            render_object.triangles = rri.triangles.load(Ordering::SeqCst);

            self.rdevice.cmd_end_render_pass(render_object.render_cb);

            self.rdevice
                .end_command_buffer(render_object.render_cb)
                .expect("Command buffer end failed");
        }
    }

    pub fn request_rerecord(&self) {
        let data = self.data.read();

        for target in data.render_objects.iter() {
            target.write().rerecord_requested = true;
        }
    }

    pub fn recreate_swapchain(&self) {
        let data = self.data.read();

        let mut render_objects = data
            .render_objects
            .iter()
            .map(|render_object| render_object.write())
            .collect::<Vec<_>>();

        let mut swapchain_objects = data.swapchain_objects.write();

        self.wait();

        self.frame.store(0, Ordering::SeqCst);

        unsafe {
            swapchain_objects
                .swapchain_loader
                .destroy_swapchain(swapchain_objects.swapchain, None)
        };
        let (swapchain, extent, viewport, scissor) = RendererBuilder::swapchain(
            self.rdevice.pdevice,
            swapchain_objects.surface,
            &swapchain_objects.surface_loader,
            &swapchain_objects.swapchain_loader,
            swapchain_objects.format,
            swapchain_objects.present,
            swapchain_objects.extent,
        )
        .unwrap();

        swapchain_objects.extent = extent;
        swapchain_objects.swapchain = swapchain;
        swapchain_objects.viewport = viewport;
        swapchain_objects.scissor = scissor;

        let color_images =
            RendererBuilder::swapchain_images(&swapchain_objects.swapchain_loader, swapchain)
                .unwrap();

        for (i, render_objects) in render_objects.iter_mut().enumerate() {
            unsafe {
                self.rdevice
                    .destroy_framebuffer(render_objects.framebuffer, None);
            }

            let color_image = ImageBuilder::new_with_device(self.rdevice.clone())
                .build_with_image(
                    color_images[i],
                    ImageUsage::WRITE,
                    swapchain_objects.format.format,
                )
                .expect("Color image creation failed");

            let depth_image = ImageBuilder::new_with_device(self.rdevice.clone())
                .with_width(swapchain_objects.extent.width)
                .with_height(swapchain_objects.extent.height)
                .build(ImageUsage::WRITE, ImageFormat::<f32>::D)
                .expect("Depth image creation failed");

            let attachments = [color_image.view(), depth_image.view()];

            let framebuffer_info = vk::FramebufferCreateInfo::builder()
                .attachments(&attachments)
                .render_pass(swapchain_objects.render_pass)
                .width(swapchain_objects.extent.width)
                .height(swapchain_objects.extent.height)
                .layers(1);

            render_objects.framebuffer =
                unsafe { self.rdevice.create_framebuffer(&framebuffer_info, None) }
                    .expect("Framebuffer creation failed");

            render_objects._color_image = color_image;
            render_objects._depth_image = depth_image;
        }

        self.request_rerecord();
    }

    pub fn frames_in_flight(&self) -> usize {
        self.frames_in_flight
    }

    pub fn wait(&self) {
        let queue_wait_result = |res: Result<(), vk::Result>| {
            res.map_err_else_log("Could not wait for queue to become idle", |err| match err {
                vk::Result::ERROR_DEVICE_LOST => ContextError::DriverCrash,
                _ => ContextError::OutOfMemory,
            })
            .unwrap()
        };

        unsafe {
            queue_wait_result(self.rdevice.queue_wait_idle(self.rdevice.queues.graphics));
            queue_wait_result(self.rdevice.queue_wait_idle(self.rdevice.queues.present));
        }
    }
}

impl RendererBuilder {
    /// Limits the framerate to usually 60 depending on the display settings.
    ///
    /// Eliminates screen tearing.
    pub fn with_sync(mut self, sync: SyncMode) -> Self {
        self.sync = sync;
        self
    }

    /// Increasing frames in flight <u>MIGHT</u> decrease the cpu frametime if the scene is simple.
    ///
    /// Slightly increases input delay.
    ///
    /// Recommended values: 2 or 3.
    ///
    /// 1 Never recommended and going above rarely improves anything.
    pub fn with_frames_in_flight(mut self, frames_in_flight: usize) -> Self {
        self.frames_in_flight = frames_in_flight;
        self
    }

    fn pick_surface_format(
        pdevice: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &khr::Surface,
    ) -> Result<vk::SurfaceFormatKHR, ContextError> {
        let available =
            unsafe { surface_loader.get_physical_device_surface_formats(pdevice, surface) }
                .map_err_log("Surface format query failed", ContextError::OutOfMemory)?;

        if available.len() == 0 {
            error!("No surface formats available");
            return Err(ContextError::MissingSurfaceConfigs);
        }

        let format = available
            .iter()
            .find(|format| {
                format.format == vk::Format::R8G8B8A8_SRGB
                    && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or(&available[0]);
        let format = format.clone();

        debug!("Surface format chosen: {:?} from {:?}", format, available);

        Ok(format)
    }

    fn pick_surface_present_mode(
        pdevice: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &khr::Surface,
        vsync: SyncMode,
    ) -> Result<vk::PresentModeKHR, ContextError> {
        let available =
            unsafe { surface_loader.get_physical_device_surface_present_modes(pdevice, surface) }
                .map_err_log(
                "Surface present mode query failed",
                ContextError::OutOfMemory,
            )?;

        if available.len() == 0 {
            error!("No surface present modes available");
            return Err(ContextError::MissingSurfaceConfigs);
        }

        let mode = match vsync {
            SyncMode::Fifo => vk::PresentModeKHR::FIFO,
            SyncMode::Immediate => available
                .iter()
                .find(|&&present| present == vk::PresentModeKHR::IMMEDIATE)
                .unwrap_or(&vk::PresentModeKHR::FIFO)
                .clone(),
            SyncMode::Mailbox => available
                .iter()
                .find(|&&present| present == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(&vk::PresentModeKHR::FIFO)
                .clone(),
        };

        debug!(
            "Surface present mode chosen: {:?} from {:?}",
            mode, available
        );

        Ok(mode)
    }

    fn swapchain_len(surface_caps: &vk::SurfaceCapabilitiesKHR) -> u32 {
        let preferred = surface_caps.min_image_count + 1;

        if surface_caps.max_image_count != 0 {
            preferred.min(surface_caps.max_image_count)
        } else {
            preferred
        }
    }

    fn swapchain_extent(
        initial_extent: vk::Extent2D,
        surface_caps: &vk::SurfaceCapabilitiesKHR,
    ) -> vk::Extent2D {
        if surface_caps.current_extent.width != u32::MAX {
            surface_caps.current_extent
        } else {
            vk::Extent2D {
                width: initial_extent
                    .width
                    .max(surface_caps.min_image_extent.width)
                    .min(surface_caps.max_image_extent.width),
                height: initial_extent
                    .height
                    .max(surface_caps.min_image_extent.height)
                    .min(surface_caps.max_image_extent.height),
            }
        }
    }

    fn swapchain(
        pdevice: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &khr::Surface,
        swapchain_loader: &khr::Swapchain,
        format: vk::SurfaceFormatKHR,
        present: vk::PresentModeKHR,
        initial_extent: vk::Extent2D,
    ) -> Result<(vk::SwapchainKHR, vk::Extent2D, vk::Viewport, vk::Rect2D), ContextError> {
        let surface_caps =
            unsafe { surface_loader.get_physical_device_surface_capabilities(pdevice, surface) }
                .map_err_else_log("Surface capability query failed", |err| match err {
                    vk::Result::ERROR_SURFACE_LOST_KHR => ContextError::FrameLost,
                    _ => ContextError::OutOfMemory,
                })?;

        let min_swapchain_len = Self::swapchain_len(&surface_caps);

        let extent = Self::swapchain_extent(initial_extent, &surface_caps);

        let transform = if surface_caps
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            surface_caps.current_transform
        };

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(min_swapchain_len)
            .image_color_space(format.color_space)
            .image_format(format.format)
            .image_extent(extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present)
            .clipped(true)
            .image_array_layers(1);

        debug!(
            "Swapchain images: {} - Swapchain format: {:?}",
            min_swapchain_len, format
        );

        let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None) }
            .map_err_else_log("Swapchain creation failed", |err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                    ContextError::OutOfMemory
                }
                vk::Result::ERROR_DEVICE_LOST => ContextError::DriverCrash,
                _ => ContextError::FrameInUse,
            })?;

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as f32,
            height: extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: extent.clone(),
        };

        Ok((swapchain, extent, viewport, scissor))
    }

    fn swapchain_images(
        swapchain_loader: &khr::Swapchain,
        swapchain: vk::SwapchainKHR,
    ) -> Result<Vec<vk::Image>, ContextError> {
        unsafe { swapchain_loader.get_swapchain_images(swapchain) }
            .map_err_log("Swapchain image query failed", ContextError::OutOfMemory)
    }

    fn render_pass(
        device: Arc<RenderDevice>,
        format: vk::Format,
    ) -> Result<vk::RenderPass, ContextError> {
        let color_attachment = vk::AttachmentDescription::builder()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build();

        let color_attachment_ref = [vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()];

        let depth_attachment = vk::AttachmentDescription::builder()
            .format(ImageFormat::<f32>::D.format())
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        let depth_attachment_ref = vk::AttachmentReference::builder()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        let dependencies = [vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            )
            .build()];

        let attachments = [color_attachment, depth_attachment];

        let subpasses = [vk::SubpassDescription::builder()
            .color_attachments(&color_attachment_ref)
            .depth_stencil_attachment(&depth_attachment_ref)
            .build()];

        let render_pass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        unsafe { device.create_render_pass(&render_pass_info, None) }
            .map_err_log("Render pass creation failed", ContextError::OutOfMemory)
    }

    pub fn build(self, context: Context) -> Result<Renderer, ContextError> {
        debug!("Renderer created");

        // rdevice
        let (r_context, surface, surface_loader, mut extent) = ReducedContext::new(context);
        let rdevice = RenderDevice::from_context(r_context)?;

        // swapchain
        let format = Self::pick_surface_format(rdevice.pdevice, surface, &surface_loader)?;
        let present =
            Self::pick_surface_present_mode(rdevice.pdevice, surface, &surface_loader, self.sync)?;

        let swapchain_loader = khr::Swapchain::new(&rdevice.instance, &**rdevice);

        let (swapchain, new_extent, viewport, scissor) = Self::swapchain(
            rdevice.pdevice,
            surface,
            &surface_loader,
            &swapchain_loader,
            format,
            present,
            extent,
        )?;
        extent = new_extent;

        let color_images = Self::swapchain_images(&swapchain_loader, swapchain)?;

        // main render pass
        let render_pass = Self::render_pass(rdevice.clone(), format.format)?;

        let render_objects = color_images
            .into_iter()
            .map(|image| {
                Ok(RwLock::new(RenderObject::new(
                    rdevice.clone(),
                    render_pass,
                    image,
                    format.format,
                    extent,
                )?))
            })
            .collect::<Result<_, _>>()?;

        let frames_in_flight = self.frames_in_flight;
        let crender_objects = (0..frames_in_flight)
            .map(|_| Ok(RwLock::new(ConcurrentRenderObject::new(&rdevice)?)))
            .collect::<Result<_, _>>()?;

        let swapchain_objects = RwLock::new(SwapchainObjects {
            extent,
            format,
            present,
            viewport,
            scissor,

            render_pass,

            swapchain_loader,
            swapchain,

            surface_loader,
            surface,
        });

        let data = Arc::new(RwLock::new(RendererData {
            swapchain_objects,
            render_objects,
            crender_objects,
        }));

        /* let (main_thread_tx, present_thread_rx) = mpsc::channel();
        let (present_thread_tx, main_thread_rx) = mpsc::channel();
        let main_thread_tx = Mutex::new(main_thread_tx);
        let main_thread_rx = Mutex::new(main_thread_rx);

        let present_thread_join = {
            let data = data.clone();
            let rdevice = rdevice.clone();

            Some(thread::spawn(move || loop {
                let event = present_thread_rx.recv().unwrap();

                match event {
                    PresentThreadEvent::Stop => {
                        break;
                    }
                    PresentThreadEvent::PresentImage(frame, image_index) => {
                        let data = data.read();
                        let swapchain_objects = data.swapchain_objects.read();
                        let crender_object = data.crender_objects[frame as usize].read();
                        let wait = [crender_object.render_semaphore];
                        let swapchain = [swapchain_objects.swapchain];
                        let image_index = [image_index];

                        let submit_present = vk::PresentInfoKHR::builder()
                            .wait_semaphores(&wait)
                            .swapchains(&swapchain)
                            .image_indices(&image_index);

                        let result = unsafe {
                            // present
                            let result = swapchain_objects
                                .swapchain_loader
                                .queue_present(rdevice.queues.present, &submit_present);
                            let current = data.images.fetch_sub(1, Ordering::SeqCst);
                            debug!("queue_present result: {:?}, {}", result, current);
                            match result {
                                Ok(o) => o,
                                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
                                Err(e) => panic!("Present queue submit failed: {:?}", e),
                            }
                        };

                        present_thread_tx.send(result).unwrap();
                    }
                }
            }))
        }; */

        Ok(Renderer {
            /* present_thread_join,
            main_thread_tx,
            main_thread_rx, */
            data,

            frame: AtomicUsize::new(0),
            frames_in_flight,

            rdevice,
        })
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        /* self.main_thread_tx
            .lock()
            .send(PresentThreadEvent::Stop)
            .unwrap();
        self.present_thread_join.take().unwrap().join().unwrap(); */
        let mut data = self.data.write();

        self.wait();

        unsafe {
            for crender_object in data.crender_objects.drain(..) {
                let crender_object = crender_object.write();

                self.rdevice.destroy_fence(crender_object.frame_fence, None);
                self.rdevice
                    .destroy_semaphore(crender_object.image_semaphore, None);
                self.rdevice
                    .destroy_semaphore(crender_object.update_semaphore, None);
                self.rdevice
                    .destroy_semaphore(crender_object.render_semaphore, None);
            }

            for render_object in data.render_objects.drain(..) {
                let render_object = render_object.write();

                self.rdevice
                    .destroy_framebuffer(render_object.framebuffer, None);
                let cbs = [render_object.render_cb, render_object.update_cb];
                self.rdevice
                    .free_command_buffers(render_object.command_pool, &cbs);
                self.rdevice
                    .destroy_command_pool(render_object.command_pool, None);
            }

            let swapchain_objects = data.swapchain_objects.write();
            self.rdevice
                .destroy_render_pass(swapchain_objects.render_pass, None);
            swapchain_objects
                .swapchain_loader
                .destroy_swapchain(swapchain_objects.swapchain, None);
            swapchain_objects
                .surface_loader
                .destroy_surface(swapchain_objects.surface, None);
        }
        debug!("Renderer dropped");
    }
}
