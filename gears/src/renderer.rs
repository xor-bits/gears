pub mod buffer;
pub mod device;
pub mod object;
pub mod pipeline;
pub mod query;
pub mod queue;
pub mod simple_renderer;
pub mod target;

use self::query::PerfQueryResult;
use glam::Vec4;
use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

pub struct FramePerfReport {
    pub cpu_frame_time: Duration,
    pub gpu_frame_time: PerfQueryResult,
}

impl Default for FramePerfReport {
    fn default() -> Self {
        Self {
            cpu_frame_time: Duration::from_secs(0),
            gpu_frame_time: PerfQueryResult::default(),
        }
    }
}

pub type BeginInfoRecorder<'a> = (
    &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ClearColor,
);

struct RecorderInner {
    command_buffer: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    modified: bool,

    image_index: usize,
    /* frame_in_flight: usize, */
}

impl RecorderInner {
    pub fn record(&mut self) -> &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer> {
        self.modified = true;
        &mut self.command_buffer
    }
}

struct RecorderTarget<'a> {
    target: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
}

impl<'a> Deref for RecorderTarget<'a> {
    type Target = AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>;

    fn deref(&self) -> &Self::Target {
        self.target
    }
}

impl<'a> DerefMut for RecorderTarget<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.target
    }
}

pub struct Recorder<const IN_RENDER_PASS: bool> {
    inner: RecorderInner,
    begin_info: Box<dyn Fn(BeginInfoRecorder)>,
}

impl<const IN_RENDER_PASS: bool> Recorder<IN_RENDER_PASS> {
    pub fn new(
        command_buffer: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        begin_info: impl Fn(BeginInfoRecorder) + 'static,
        image_index: usize,
        /* frame_in_flight: usize, */
    ) -> Self {
        let begin_info = Box::new(begin_info);

        Self {
            inner: RecorderInner {
                command_buffer,
                modified: false,

                image_index,
                /* frame_in_flight, */
            },
            begin_info,
        }
    }

    pub fn image_index(&self) -> usize {
        self.inner.image_index
    }

    /* pub fn frame_in_flight(&self) -> usize {
        self.inner.frame_in_flight
    } */

    pub fn record(&mut self) -> &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer> {
        self.inner.record()
    }
}

impl Recorder<false> {
    pub fn begin_render_pass(self) -> Recorder<true> {
        self.begin_render_pass_with(ClearColor::default())
    }

    pub fn begin_render_pass_with(mut self, cc: ClearColor) -> Recorder<true> {
        let f = self.begin_info;
        f((self.inner.record(), cc));
        self.begin_info = f;
        Recorder::<true> {
            inner: self.inner,
            begin_info: self.begin_info,
        }
    }
}

impl Recorder<true> {
    pub fn end_render_pass(mut self) -> Recorder<false> {
        self.record().end_render_pass().unwrap();
        Recorder::<false> {
            inner: self.inner,
            begin_info: self.begin_info,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClearColor(Vec4);

impl ClearColor {
    fn c(&self) -> [f32; 4] {
        self.0.to_array()
    }
}

impl Default for ClearColor {
    fn default() -> Self {
        Self(Vec4::new(0.16, 0.18, 0.2, 1.0))
    }
}
