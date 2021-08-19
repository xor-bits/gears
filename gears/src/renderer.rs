// TODO: pub mod batch;
pub mod device;
pub mod object;
pub mod query;
pub mod queue;
pub mod simple_renderer;
pub mod target;
/* pub mod buffer; */
/* pub mod pipeline; */
/* pub mod render_pass; */
/* pub mod surface; */
/* pub mod swapchain; */

use self::query::PerfQueryResult;
use glam::Vec4;
use std::{sync::atomic::AtomicUsize, time::Duration};
use vulkano::command_buffer::PrimaryAutoCommandBuffer;

pub struct FramePerfReport {
    pub cpu_frametime: Duration,
    pub gpu_frametime: PerfQueryResult,

    pub rerecord: bool,
    pub updates: bool,
    pub triangles: usize,
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

pub struct ImmediateFrameInfo {
    pub image_index: usize,
}

pub struct RenderRecordInfo {
    command_buffer: PrimaryAutoCommandBuffer,
    image_index: usize,
    triangles: AtomicUsize,
    debug_calls: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RenderRecordBeginInfo {
    pub debug_calls: bool,
    pub clear_color: Vec4,
}

pub struct UpdateRecordInfo {
    command_buffer: PrimaryAutoCommandBuffer,
    image_index: usize,
}

pub trait RendererRecord {
    #[allow(unused_variables)]
    fn immediate(&self, imfi: &ImmediateFrameInfo) {}

    #[allow(unused_variables)]
    unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        // 'any' all object updates and return the result of that
        false
    }

    #[allow(unused_variables)]
    fn begin_info(&self) -> RenderRecordBeginInfo {
        RenderRecordBeginInfo {
            clear_color: Vec4::new(0.18, 0.18, 0.2, 1.0),
            debug_calls: false,
        }
    }

    #[allow(unused_variables)]
    unsafe fn record(&self, rri: &RenderRecordInfo) {}
}
