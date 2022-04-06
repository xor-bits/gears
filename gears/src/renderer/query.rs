use super::{device::Dev, Recorder};
use std::{sync::Arc, time::Duration};
use vulkano::{
    command_buffer::AutoCommandBufferBuilder,
    query::{GetResultsError, QueryPool, QueryPoolCreateInfo, QueryResultFlags, QueryType},
    sync::PipelineStage,
};

//

#[derive(Debug)]
pub enum PerfQueryError {
    GetResultsError(GetResultsError),
}

pub struct PerfQuery {
    query_pool: Arc<QueryPool>,
}

pub trait RecordPerf {
    fn reset_perf(&mut self, perf: &PerfQuery) -> &'_ mut Self;
    fn begin_perf(&mut self, perf: &PerfQuery) -> &'_ mut Self;
    fn end_perf(&mut self, perf: &PerfQuery) -> &'_ mut Self;
}

//

impl PerfQuery {
    pub fn new_with_device(device: &Dev) -> Self {
        let info = QueryPoolCreateInfo {
            query_count: 2,
            ..QueryPoolCreateInfo::query_type(QueryType::Timestamp)
        };
        let query_pool =
            QueryPool::new(device.logical().clone(), info).expect("Could not create a query pool");

        Self { query_pool }
    }

    pub fn reset(&self, recorder: &mut Recorder<false>) {
        recorder.record().reset_perf(self);
    }

    pub fn begin(&self, recorder: &mut Recorder<true>) {
        recorder.record().begin_perf(self);
    }

    pub fn end(&self, recorder: &mut Recorder<true>) {
        recorder.record().end_perf(self);
    }

    pub fn get(&self) -> Option<Duration> {
        let mut data = [0_u64; 2];
        match self.query_pool.queries_range(0..2).unwrap().get_results(
            &mut data,
            QueryResultFlags {
                wait: false,
                with_availability: false,
                partial: false,
            },
        ) {
            Ok(true) => {}
            Ok(false) => return None,
            Err(err) => panic!("{}", err),
        };

        let pipeline_begin = data[0];
        let pipeline_end = data[1];

        Some(Duration::from_nanos(
            pipeline_end.saturating_sub(pipeline_begin),
        ))
    }
}

impl<L, P> RecordPerf for AutoCommandBufferBuilder<L, P> {
    fn reset_perf(&mut self, perf: &PerfQuery) -> &'_ mut Self {
        // TODO: get rid of this unsafe
        unsafe {
            self.reset_query_pool(perf.query_pool.clone(), 0..2)
                .unwrap();
        }
        self
    }

    fn begin_perf(&mut self, perf: &PerfQuery) -> &'_ mut Self {
        // TODO: get rid of this unsafe
        unsafe {
            self.write_timestamp(perf.query_pool.clone(), 0, PipelineStage::TopOfPipe)
                .unwrap();
        }
        self
    }

    fn end_perf(&mut self, perf: &PerfQuery) -> &'_ mut Self {
        // TODO: get rid of this unsafe
        unsafe {
            self.write_timestamp(perf.query_pool.clone(), 1, PipelineStage::BottomOfPipe)
                .unwrap();
        }
        self
    }
}
