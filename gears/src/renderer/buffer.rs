use gfx_hal::{
    adapter::MemoryType,
    buffer::{SubRange, Usage},
    command::CommandBuffer,
    device::{BindError, Device},
    memory::{Properties, Requirements, Segment},
    Backend, MemoryTypeId,
};
use log::debug;

use std::{iter, mem, ptr};

pub trait Buffer<B: Backend> {
    fn new<T>(
        device: &B::Device,
        buffer_manager: &mut BufferManager,
        available_memory_types: &Vec<MemoryType>,
        size: usize,
    ) -> Self;
    fn bind(&self, command_buffer: &mut B::CommandBuffer);
    fn destroy(self, device: &B::Device);
}

type BufferID = usize;

pub struct BufferManager {
    next_id: BufferID,
    bound_id: BufferID,
}

pub struct VertexBuffer<B: Backend> {
    buffer: B::Buffer,
    memory: B::Memory,
    len: usize,
    id: BufferID,
}

impl BufferManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            bound_id: 0,
        }
    }

    fn next(&mut self) -> BufferID {
        let result = self.next_id;
        self.next_id += 1;
        result
    }

    fn bind_memory<B: Backend>(
        &mut self,
        device: &B::Device,
        memory: &B::Memory,
        offset: u64,
        buffer: &mut B::Buffer,
        id: BufferID,
    ) -> Result<(), BindError> {
        if self.bound_id != id {
            self.bound_id = id;
            debug!("Binding memory: {}", id);
            unsafe { device.bind_buffer_memory(memory, offset, buffer) }
        } else {
            Ok(())
        }
    }
}

impl<B: Backend> VertexBuffer<B> {
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
}

impl<B: Backend> Buffer<B> for VertexBuffer<B> {
    // size = vertex count NOT byte count
    fn new<T>(
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
                    Properties::CPU_VISIBLE, /* | Properties::COHERENT */
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

    fn bind(&self, command_buffer: &mut B::CommandBuffer) {
        unsafe {
            command_buffer.bind_vertex_buffers(0, iter::once((&self.buffer, SubRange::WHOLE)));
        }
    }

    fn destroy(self, device: &B::Device) {
        unsafe {
            device.free_memory(self.memory);
            device.destroy_buffer(self.buffer);
        }
    }
}

fn upload_type(
    available_memory_types: &Vec<MemoryType>,
    requirements: &Requirements,
    properties: Properties,
) -> MemoryTypeId {
    available_memory_types
        .iter()
        .enumerate()
        .position(|(id, mem_type)| {
            // type_mask is a bit field where each bit represents a memory type. If the bit is set
            // to 1 it means we can use that type for our buffer. So this code finds the first
            // memory type that has a `1` (or, is allowed), and is visible to the CPU.
            requirements.type_mask & (1 << id) != 0 && mem_type.properties.contains(properties)
        })
        .unwrap()
        .into()
}
