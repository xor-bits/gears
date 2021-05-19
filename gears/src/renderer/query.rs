use ash::{version::DeviceV1_0, vk};
use log::debug;
use std::{
    ops::{Add, AddAssign},
    sync::Arc,
    time::Duration,
};

use crate::renderer::RenderRecordInfo;

use super::device::RenderDevice;

const TIMESTAMP_STAGES: [vk::PipelineStageFlags; 6] = [
    vk::PipelineStageFlags::BOTTOM_OF_PIPE, // pipeline begin
    vk::PipelineStageFlags::VERTEX_SHADER,  // vertex begin
    vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER, // vertex end
    vk::PipelineStageFlags::FRAGMENT_SHADER, // fragment begin
    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, // fragment end
    vk::PipelineStageFlags::TOP_OF_PIPE,    // pipeline end
];
const TIMESTAMP_COUNT: u32 = TIMESTAMP_STAGES.len() as u32;

pub struct PerfQueryResult {
    pub whole_pipeline: Duration,

    pub vertex: Duration,
    pub fragment: Duration,
}

#[derive(Debug)]
pub enum PerfQueryError {
    NotDone,
    WaitError,
}

pub struct PerfQuery {
    device: Arc<RenderDevice>,
    query_pool: vk::QueryPool,
}

impl Default for PerfQueryResult {
    fn default() -> Self {
        Self {
            whole_pipeline: Duration::from_secs(0),
            vertex: Duration::from_secs(0),
            fragment: Duration::from_secs(0),
        }
    }
}

impl AddAssign for PerfQueryResult {
    fn add_assign(&mut self, rhs: Self) {
        self.whole_pipeline += rhs.whole_pipeline;
        self.vertex += rhs.vertex;
        self.fragment += rhs.fragment;
    }
}

impl Add for PerfQueryResult {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl PerfQuery {
    pub fn new_with_device(device: Arc<RenderDevice>) -> Self {
        let query_pool_info = vk::QueryPoolCreateInfo::builder()
            .query_type(vk::QueryType::TIMESTAMP)
            .query_count(TIMESTAMP_COUNT);

        // Unsafe: device must be valid
        let query_pool = unsafe { device.create_query_pool(&query_pool_info, None) }
            .expect("Could not create a query pool");

        Self { device, query_pool }
    }

    pub unsafe fn reset(&self, rri: &RenderRecordInfo) {
        if rri.debug_calls {
            debug!("cmd_reset_query_pool");
        }

        self.device
            .cmd_reset_query_pool(rri.command_buffer, self.query_pool, 0, TIMESTAMP_COUNT);
    }

    unsafe fn query(&self, rri: &RenderRecordInfo, id: u32) {
        if rri.debug_calls {
            debug!("cmd_write_timestamp");
        }

        self.device.cmd_write_timestamp(
            rri.command_buffer,
            TIMESTAMP_STAGES[id as usize],
            self.query_pool,
            id,
        );
    }

    pub unsafe fn bind(&self, rri: &RenderRecordInfo) {
        for i in 0..TIMESTAMP_COUNT {
            self.query(rri, i);
        }
    }

    pub fn get_with_flags(
        &self,
        flags: vk::QueryResultFlags,
    ) -> Result<PerfQueryResult, PerfQueryError> {
        let mut data = [0u64; TIMESTAMP_STAGES.len()];

        unsafe {
            self.device.get_query_pool_results(
                self.query_pool,
                0,
                TIMESTAMP_COUNT,
                &mut data,
                flags,
            )
        }
        .or(Err(PerfQueryError::WaitError))?;

        let pipeline_begin = data[0 as usize];
        let pipeline_end = data[5 as usize];

        let vertex_begin = data[1 as usize];
        let vertex_end = data[2 as usize];

        let fragment_begin = data[3 as usize];
        let fragment_end = data[4 as usize];

        Ok(PerfQueryResult {
            whole_pipeline: Duration::from_nanos(pipeline_end - pipeline_begin),
            vertex: Duration::from_nanos(vertex_end - vertex_begin),
            fragment: Duration::from_nanos(fragment_end - fragment_begin),
        })
    }

    pub fn get(&mut self) -> Result<PerfQueryResult, PerfQueryError> {
        self.get_with_flags(vk::QueryResultFlags::TYPE_64)
    }

    /* pub fn get_wait(&mut self) -> Result<PerfQueryResult, PerfQueryError> {
        self.get_with_flags(vk::QueryResultFlags::WAIT | vk::QueryResultFlags::TYPE_64)
    } */
}
