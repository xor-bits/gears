use std::ops::{Deref, DerefMut};

use super::BufferError;
use crate::{DerefDev, GenericBuffer, WriteBuffer, WriteType};
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

const USAGE: u32 = vk::BufferUsageFlags::INDIRECT_BUFFER.as_raw();
const MULTI: bool = false;

type InternalIndirectBuffer = GenericBuffer<CountData, USAGE, MULTI>;

pub struct IndirectBuffer {
    buffer: InternalIndirectBuffer,
    data: CountData,
}

impl IndirectBuffer {
    pub fn new<D>(device: &D) -> Result<Self, BufferError>
    where
        D: DerefDev,
    {
        Ok(Self {
            buffer: InternalIndirectBuffer::new_single(device)?,
            data: CountData::new(0, 0),
        })
    }

    pub fn new_with<D>(device: &D, count: u32, offset: u32) -> Result<Self, BufferError>
    where
        D: DerefDev,
    {
        let mut buffer = Self::new(device)?;
        buffer.write(count, offset)?;
        Ok(buffer)
    }

    pub fn write(&mut self, count: u32, offset: u32) -> Result<WriteType, BufferError> {
        self.data.set(count, offset);
        let data = self.data;
        self.buffer.write(&data)
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

impl Deref for IndirectBuffer {
    type Target = InternalIndirectBuffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for IndirectBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}
