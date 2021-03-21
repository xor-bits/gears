use std::{
    iter,
    mem::{self, ManuallyDrop},
    ptr,
    sync::Arc,
};

use gfx_hal::{
    adapter::MemoryType,
    buffer::{SubRange, Usage},
    command::CommandBuffer,
    device::Device,
    memory::{Properties, Segment},
    Backend, IndexType,
};

use crate::GearsRenderer;

use super::upload_type;

pub struct IndexBuffer<B: Backend> {
    device: Arc<B::Device>,

    buffer: ManuallyDrop<B::Buffer>,
    memory: ManuallyDrop<B::Memory>,

    len: usize,
    count: usize,
}

impl<B: Backend> IndexBuffer<B> {
    pub fn new(renderer: &GearsRenderer<B>, size: usize) -> Self {
        Self::new_with_device(renderer.device.clone(), &renderer.memory_types, size)
    }

    pub fn new_with_data(renderer: &GearsRenderer<B>, data: &[u32]) -> Self {
        let mut buffer = Self::new(renderer, data.len());
        buffer.write(0, data);
        buffer
    }

    pub fn new_with_device(
        device: Arc<B::Device>,
        available_memory_types: &Vec<MemoryType>,
        size: usize,
    ) -> Self {
        let len = size * mem::size_of::<u32>();
        let mut buffer =
            ManuallyDrop::new(unsafe { device.create_buffer(len as u64, Usage::INDEX) }.unwrap());
        let req = unsafe { device.get_buffer_requirements(&buffer) };

        let memory = ManuallyDrop::new(
            unsafe {
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
            .unwrap(),
        );
        unsafe { device.bind_buffer_memory(&memory, 0, &mut buffer) }.unwrap();

        Self {
            device,
            buffer,
            memory,
            len,
            count: 0,
        }
    }

    pub fn write(&mut self, offset: usize, data: &[u32]) {
        unsafe {
            // map
            let mapping = self
                .device
                .map_memory(&mut self.memory, Segment::ALL)
                .unwrap();

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
            self.device
                .flush_mapped_memory_ranges(iter::once((&*self.memory, Segment::ALL)))
                .unwrap();

            // unmap
            self.device.unmap_memory(&mut self.memory);
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

impl<B: Backend> Drop for IndexBuffer<B> {
    fn drop(&mut self) {
        unsafe {
            let memory = ManuallyDrop::into_inner(ptr::read(&self.memory));
            self.device.free_memory(memory);

            let buffer = ManuallyDrop::into_inner(ptr::read(&self.buffer));
            self.device.destroy_buffer(buffer);
        }
    }
}
