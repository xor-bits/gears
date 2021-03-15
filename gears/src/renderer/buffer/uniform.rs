use std::mem;

use gfx_hal::{buffer::Usage, device::Device, memory::Properties, Backend};

use super::{upload_type, Buffer};

pub struct UniformBuffer<B: Backend> {
    buffer: B::Buffer,
    memory: B::Memory,
}

impl<B: Backend> UniformBuffer<B> {
    pub fn new<T>(
        device: &B::Device,
        available_memory_types: &Vec<gfx_hal::adapter::MemoryType>,
        size: usize,
    ) -> Self {
        let len = size * mem::size_of::<T>();
        let buffer = unsafe { device.create_buffer(len as u64, Usage::UNIFORM) }.unwrap();
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

        Self { buffer, memory }
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
