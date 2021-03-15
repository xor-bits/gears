use std::{iter, mem, ptr};

use gfx_hal::{
    adapter::MemoryType,
    buffer::{SubRange, Usage},
    command::CommandBuffer,
    device::Device,
    memory::{Properties, Segment},
    Backend,
};

use super::{upload_type, Buffer, BufferID, BufferManager};

pub struct VertexBuffer<B: Backend> {
    buffer: B::Buffer,
    memory: B::Memory,
    len: usize,
    id: BufferID,
}

impl<B: Backend> VertexBuffer<B> {
    // size = vertex count NOT byte count
    pub fn new<T>(
        device: &B::Device,
        buffer_manager: &mut BufferManager,
        available_memory_types: &Vec<MemoryType>,
        size: usize,
    ) -> Self {
        let len = size * mem::size_of::<T>();
        let buffer = unsafe { device.create_buffer(len as u64, Usage::VERTEX) }.unwrap();
        let vertex_buffer_req = unsafe { device.get_buffer_requirements(&buffer) };

        let memory = unsafe {
            device.allocate_memory(
                upload_type(
                    available_memory_types,
                    &vertex_buffer_req,
                    Properties::CPU_VISIBLE | Properties::COHERENT,
                    Properties::CPU_VISIBLE,
                ),
                vertex_buffer_req.size,
            )
        }
        .unwrap();

        Self {
            buffer,
            memory,
            len,
            id: buffer_manager.next(),
        }
    }

    pub fn write<T>(
        &mut self,
        device: &B::Device,
        buffer_manager: &mut BufferManager,
        offset: usize,
        data: &[T],
    ) {
        unsafe {
            // map
            buffer_manager
                .bind_memory::<B>(device, &self.memory, 0, &mut self.buffer, self.id)
                .unwrap();
            let mapping = device.map_memory(&mut self.memory, Segment::ALL).unwrap();

            let written_len = mem::size_of::<T>() * data.len();
            assert!(
                offset + written_len <= self.len,
                "Tried to overflow the buffer"
            );

            // write
            ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                mapping,
                mem::size_of::<T>() * data.len(),
            );
            device
                .flush_mapped_memory_ranges(iter::once((&self.memory, Segment::ALL)))
                .unwrap();

            // unmap
            device.unmap_memory(&mut self.memory);
        }
    }

    pub fn bind(&self, command_buffer: &mut B::CommandBuffer) {
        unsafe {
            command_buffer.bind_vertex_buffers(0, iter::once((&self.buffer, SubRange::WHOLE)));
        }
    }
}

impl<B: Backend> Buffer<B> for VertexBuffer<B> {
    fn destroy(self, device: &B::Device) {
        unsafe {
            device.free_memory(self.memory);
            device.destroy_buffer(self.buffer);
        }
    }
}
