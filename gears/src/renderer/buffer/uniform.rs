use gfx_hal::{
    buffer::Usage,
    device::Device,
    memory::{Properties, Segment},
    Backend,
};
use std::{
    iter,
    mem::{self, ManuallyDrop},
    ptr,
    sync::Arc,
};

use crate::GearsRenderer;

use super::upload_type;

pub struct UniformBuffer<B: Backend> {
    device: Arc<B::Device>,

    buffer: ManuallyDrop<B::Buffer>,
    memory: ManuallyDrop<B::Memory>,

    len: usize,
    count: usize,
}

impl<B: Backend> UniformBuffer<B> {
    pub fn new<T>(renderer: &GearsRenderer<B>, size: usize) -> Self {
        Self::new_with_device::<T>(renderer.device.clone(), &renderer.memory_types, size)
    }

    pub fn new_with_data<T>(renderer: &GearsRenderer<B>, data: &[T]) -> Self {
        let mut buffer = Self::new::<T>(renderer, data.len());
        buffer.write(0, data);
        buffer
    }

    pub fn new_with_device<T>(
        device: Arc<B::Device>,
        available_memory_types: &Vec<gfx_hal::adapter::MemoryType>,
        size: usize,
    ) -> Self {
        let len = size * mem::size_of::<T>();
        let mut buffer =
            ManuallyDrop::new(unsafe { device.create_buffer(len as u64, Usage::UNIFORM) }.unwrap());
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
            len: size,
            count: 0,
        }
    }

    pub fn write<T>(&mut self, offset: usize, data: &[T]) {
        unsafe {
            // map
            let mapping = self
                .device
                .map_memory(&mut self.memory, Segment::ALL)
                .unwrap();

            self.count = data.len();
            assert!(
                offset + mem::size_of::<T>() * self.count <= self.len,
                "Tried to overflow the buffer"
            );

            // write
            ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                mapping,
                mem::size_of::<T>() * data.len(),
            );
            self.device
                .flush_mapped_memory_ranges(iter::once((&*self.memory, Segment::ALL)))
                .unwrap();

            // unmap
            self.device.unmap_memory(&mut self.memory);
        }
    }

    pub fn get(&self) -> &B::Buffer {
        &self.buffer
    }

    pub fn size<T>(&self) -> usize {
        self.len / mem::size_of::<T>()
    }
}

impl<B: Backend> Drop for UniformBuffer<B> {
    fn drop(&mut self) {
        unsafe {
            let memory = ManuallyDrop::into_inner(ptr::read(&self.memory));
            self.device.free_memory(memory);

            let buffer = ManuallyDrop::into_inner(ptr::read(&self.buffer));
            self.device.destroy_buffer(buffer);
        }
    }
}
