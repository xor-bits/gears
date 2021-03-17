use gfx_hal::{
    buffer::Usage,
    device::Device,
    memory::{Properties, Segment},
    Backend,
};
use std::{iter, mem, ptr};

use super::{upload_type, Buffer};

pub struct UniformBuffer<B: Backend> {
    buffer: B::Buffer,
    memory: B::Memory,

    len: usize,
}

impl<B: Backend> UniformBuffer<B> {
    // size is the UBO size in bytes
    pub fn new(
        device: &B::Device,
        available_memory_types: &Vec<gfx_hal::adapter::MemoryType>,
        size: usize,
    ) -> Self {
        let mut buffer = unsafe { device.create_buffer(size as u64, Usage::UNIFORM) }.unwrap();
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
        unsafe { device.bind_buffer_memory(&memory, 0, &mut buffer) }.unwrap();

        Self {
            buffer,
            memory,
            len: size,
        }
    }

    pub fn write<T>(&mut self, device: &B::Device, offset: usize, data: &[T]) {
        unsafe {
            // map
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

    pub fn get<'a>(&'a self) -> &'a B::Buffer {
        &self.buffer
    }
}

impl<B: Backend> Buffer<B> for UniformBuffer<B> {
    fn destroy(self, device: &B::Device) {
        unsafe {
            device.free_memory(self.memory);
            device.destroy_buffer(self.buffer);
        }
    }
}
