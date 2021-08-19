use super::device::Dev;
use std::{
    ops::{Add, AddAssign},
    time::Duration,
};
use vulkano::{
    query::{GetResultsError, QueryPool, QueryResultFlags, QueryType},
    sync::PipelineStage,
};

const TIMESTAMP_STAGES: [PipelineStage; 6] = [
    PipelineStage::BottomOfPipe,              // pipeline begin
    PipelineStage::VertexShader,              // vertex begin
    PipelineStage::TessellationControlShader, // vertex end
    PipelineStage::FragmentShader,            // fragment begin
    PipelineStage::ColorAttachmentOutput,     // fragment end
    PipelineStage::TopOfPipe,                 // pipeline end
];
const TIMESTAMP_COUNT: u32 = TIMESTAMP_STAGES.len() as u32;

pub struct PerfQueryResult {
    pub whole_pipeline: Duration,

    pub vertex: Duration,
    pub fragment: Duration,
}

#[derive(Debug)]
pub enum PerfQueryError {
    GetResultsError(GetResultsError),
}

pub struct PerfQuery {
    query_pool: QueryPool,
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
    pub fn new_with_device(device: &Dev) -> Self {
        let query_pool = QueryPool::new(
            device.logical().clone(),
            QueryType::Timestamp,
            TIMESTAMP_COUNT,
        )
        .expect("Could not create a query pool");

        Self { query_pool }
    }

    /* TODO: pub unsafe fn reset(&self, rri: &RenderRecordInfo) {
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
    } */

    pub fn get_with_flags(
        &self,
        flags: QueryResultFlags,
    ) -> Result<PerfQueryResult, PerfQueryError> {
        let mut data = [0u64; TIMESTAMP_STAGES.len()];

        let got_data = self
            .query_pool
            .queries_range(0..TIMESTAMP_COUNT)
            .unwrap()
            .get_results(&mut data, flags)
            .map_err(|err| PerfQueryError::GetResultsError(err))?;
        assert!(got_data);

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
        self.get_with_flags(QueryResultFlags {
            partial: false,
            wait: false,
            with_availability: false,
        })
    }

    /* pub fn get_wait(&mut self) -> Result<PerfQueryResult, PerfQueryError> {
        self.get_with_flags(QueryResultFlags {
            partial: false,
            wait: true,
            with_availability: false,
        })
    } */
}
