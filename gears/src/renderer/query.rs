use ash::{version::DeviceV1_0, vk};
use std::{sync::Arc, time::Duration};

use crate::RenderRecordInfo;

const TIMESTAMP_STAGES: [vk::PipelineStageFlags; 2] = [
    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
    vk::PipelineStageFlags::TOP_OF_PIPE,
];
const TIMESTAMP_COUNT: u32 = TIMESTAMP_STAGES.len() as u32;

#[derive(Debug)]
pub enum PerfQueryError {
    WaitError,
}

pub struct PerfQuery {
    device: Arc<ash::Device>,
    query_pool: vk::QueryPool,
}

impl PerfQuery {
    pub fn new_with_device(device: Arc<ash::Device>) -> Self {
        let query_pool_info = vk::QueryPoolCreateInfo::builder()
            .query_type(vk::QueryType::TIMESTAMP)
            .query_count(TIMESTAMP_COUNT);

        let query_pool = unsafe { device.create_query_pool(&query_pool_info, None) }
            .expect("Could not create a query pool");

        Self { device, query_pool }
    }

    fn query(&self, rri: &RenderRecordInfo, id: u32) {
        unsafe {
            self.device.cmd_write_timestamp(
                rri.command_buffer,
                TIMESTAMP_STAGES[id as usize],
                self.query_pool,
                id,
            );
        }
    }

    pub fn reset(&self, rri: &RenderRecordInfo) {
        unsafe {
            self.device.cmd_reset_query_pool(
                rri.command_buffer,
                self.query_pool,
                0,
                TIMESTAMP_COUNT,
            );
        }
    }

    pub fn begin(&self, rri: &RenderRecordInfo) {
        self.query(rri, 0)
    }

    pub fn end(&self, rri: &RenderRecordInfo) {
        self.query(rri, 1)
    }

    pub fn get_with_flags(&self, flags: vk::QueryResultFlags) -> Result<Duration, PerfQueryError> {
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

        let frametime = data[1] - data[0];

        Ok(Duration::from_nanos(frametime))
    }

    pub fn get(&self) -> Result<Duration, PerfQueryError> {
        self.get_with_flags(vk::QueryResultFlags::TYPE_64)
    }

    pub fn get_wait(&self) -> Result<Duration, PerfQueryError> {
        self.get_with_flags(vk::QueryResultFlags::WAIT | vk::QueryResultFlags::TYPE_64)
    }
}
