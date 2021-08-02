use super::{stage::StageBuffer, BufferError};
use crate::{
    renderer::{device::Dev, Renderer, UpdateRecordInfo},
    Buffer, WriteBuffer, WriteType,
};
use ash::vk;
use memoffset::offset_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct NonIndexedCountData {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct IndexedCountData {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    vertex_offset: i32,
    first_instance: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct CountData {
    non_indexed: NonIndexedCountData,
    indexed: IndexedCountData,
}

impl CountData {
    fn new(count: u32, offset: u32) -> Self {
        let mut new = Self {
            non_indexed: NonIndexedCountData {
                vertex_count: 0,
                first_vertex: 0,

                instance_count: 1,
                first_instance: 0,
            },
            indexed: IndexedCountData {
                index_count: 0,
                first_index: 0,

                instance_count: 1,
                vertex_offset: 0,
                first_instance: 0,
            },
        };
        new.set(count, offset);

        new
    }

    fn set(&mut self, count: u32, offset: u32) {
        self.non_indexed.vertex_count = count;
        self.non_indexed.first_vertex = offset;

        self.indexed.index_count = count;
        self.indexed.first_index = offset;
    }
}

pub struct IndirectBuffer {
    stage: StageBuffer<CountData>,
    data: CountData,
}

impl IndirectBuffer {
    pub fn new(renderer: &Renderer, count: u32, offset: u32) -> Result<Self, BufferError> {
        Self::new_with_device(renderer.rdevice.clone(), count, offset)
    }

    pub fn new_with_device(device: Dev, count: u32, offset: u32) -> Result<Self, BufferError> {
        Ok(Self {
            stage: StageBuffer::new_with_usage(
                device.clone(),
                vk::BufferUsageFlags::INDIRECT_BUFFER,
                1,
                true,
            )?,
            data: CountData::new(count, offset),
        })
    }

    pub fn write(&mut self, count: u32, offset: u32) -> Result<WriteType, BufferError> {
        self.data.set(count, offset);
        let data = self.data;
        (self as &mut dyn WriteBuffer<CountData>).write(&data)
    }

    pub fn count(&self) -> u32 {
        self.data.non_indexed.vertex_count
    }

    pub fn non_indexed() -> u64 {
        offset_of!(CountData, non_indexed) as u64
    }

    pub fn indexed() -> u64 {
        offset_of!(CountData, indexed) as u64
    }
}

impl WriteBuffer<CountData> for IndirectBuffer {
    fn write(&mut self, data: &CountData) -> Result<WriteType, BufferError> {
        self.stage.write_single(0, data)
    }
}

impl Buffer<CountData> for IndirectBuffer {
    unsafe fn update(&mut self, uri: &UpdateRecordInfo) -> bool {
        self.stage.update(uri)
    }

    fn buffer(&self) -> vk::Buffer {
        self.stage.buffer()
    }

    fn len(&self) -> usize {
        self.stage.len()
    }

    fn capacity(&self) -> usize {
        self.stage.capacity()
    }
}
