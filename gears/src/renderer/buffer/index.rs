use std::{iter, mem, ptr};

use gfx_hal::{
    adapter::MemoryType,
    buffer::{SubRange, Usage},
    command::CommandBuffer,
    device::Device,
    memory::{Properties, Segment},
    Backend, IndexType,
};

use super::{upload_type, Buffer};

pub struct IndexBuffer<B: Backend> {
    buffer: B::Buffer,
    memory: B::Memory,

    len: usize,
    count: usize,
}

impl<B: Backend> IndexBuffer<B> {
    // size = index count NOT byte count
    pub fn new(device: &B::Device, available_memory_types: &Vec<MemoryType>, size: usize) -> Self {
        let len = size * mem::size_of::<u32>();
        let mut buffer = unsafe { device.create_buffer(len as u64, Usage::INDEX) }.unwrap();
        let req = unsafe { device.get_buffer_requirements(&buffer) };

        let memory = unsafe {
            device.allocate_memory(
                upload_type(
                    available_memory_types,
                    &req,
                    Properties::CPU_VISIBLE | Properties::COHERENT,
                    Properties::CPU_VISIBLE,
                ),
                req.size,
            )
        }
        .unwrap();
        unsafe { device.bind_buffer_memory(&memory, 0, &mut buffer) }.unwrap();

        Self {
            buffer,
            memory,
            len,
            count: 0,
        }
    }

    pub fn write(&mut self, device: &B::Device, offset: usize, data: &[u32]) {
        unsafe {
            // map
            let mapping = device.map_memory(&mut self.memory, Segment::ALL).unwrap();

            self.count = data.len();
            assert!(
                offset + mem::size_of::<u32>() * self.count <= self.len,
                "Tried to overflow the buffer"
            );

            // write
            ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                mapping,
                mem::size_of::<u32>() * data.len(),
            );
            device
                .flush_mapped_memory_ranges(iter::once((&self.memory, Segment::ALL)))
                .unwrap();

            // unmap
            device.unmap_memory(&mut self.memory);
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn bind(&self, command_buffer: &mut B::CommandBuffer) {
        unsafe {
            command_buffer.bind_index_buffer(&self.buffer, SubRange::WHOLE, IndexType::U32);
        }
    }
}

impl<B: Backend> Buffer<B> for IndexBuffer<B> {
    fn destroy(self, device: &B::Device) {
        unsafe {
            device.free_memory(self.memory);
            device.destroy_buffer(self.buffer);
        }
    }
}
