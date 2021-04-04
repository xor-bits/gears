use gfx_hal::{
    buffer::Usage,
    device::Device,
    memory::{Properties, Segment},
    Backend,
};
use std::{
    iter,
    marker::PhantomData,
    mem::{self, ManuallyDrop},
    ptr,
    sync::Arc,
};

use crate::GearsRenderer;

use super::{upload_type, BufferError};

pub struct UniformBuffer<T, B: Backend> {
    device: Arc<B::Device>,

    buffer: ManuallyDrop<B::Buffer>,
    memory: ManuallyDrop<B::Memory>,

    _p: PhantomData<T>,
}

pub trait GenericUniformBuffer<B: Backend> {
    fn write_bytes(&mut self, data: *const u8, count: usize);
    fn get(&self) -> &B::Buffer;
}

impl<T, B: Backend> UniformBuffer<T, B> {
    pub fn new(renderer: &GearsRenderer<B>) -> Result<Self, BufferError> {
        Self::new_with_device(renderer.device.clone(), &renderer.memory_types)
    }

    pub fn new_with_data(renderer: &GearsRenderer<B>, data: &T) -> Result<Self, BufferError> {
        let mut buffer = Self::new(renderer)?;
        buffer.write(data);
        Ok(buffer)
    }

    pub fn new_with_device(
        device: Arc<B::Device>,
        available_memory_types: &Vec<gfx_hal::adapter::MemoryType>,
    ) -> Result<Self, BufferError> {
        let byte_len = mem::size_of::<u32>();
        let mut buffer = ManuallyDrop::new(
            unsafe { device.create_buffer(byte_len as u64, Usage::UNIFORM) }
                .or(Err(BufferError::OutOfMemory))?,
        );
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
            .or(Err(BufferError::OutOfMemory))?,
        );
        unsafe { device.bind_buffer_memory(&memory, 0, &mut buffer) }
            .or(Err(BufferError::OutOfMemory))?;

        Ok(Self {
            device,
            buffer,
            memory,
            _p: PhantomData::default(),
        })
    }

    pub fn write(&mut self, data: &T) {
        self.write_bytes(data as *const T as *const u8, mem::size_of::<T>());
    }
}

impl<T, B: Backend> GenericUniformBuffer<B> for UniformBuffer<T, B> {
    fn write_bytes(&mut self, data: *const u8, count: usize) {
        unsafe {
            // map
            let mapping = self
                .device
                .map_memory(&mut self.memory, Segment::ALL)
                .unwrap();
            // write
            ptr::copy_nonoverlapping(data, mapping, count);
            // flush
            self.device
                .flush_mapped_memory_ranges(iter::once((&*self.memory, Segment::ALL)))
                .unwrap();
            // unmap
            self.device.unmap_memory(&mut self.memory);
        }
    }

    fn get(&self) -> &B::Buffer {
        &self.buffer
    }
}

impl<T, B: Backend> Drop for UniformBuffer<T, B> {
    fn drop(&mut self) {
        unsafe {
            let memory = ManuallyDrop::into_inner(ptr::read(&self.memory));
            self.device.free_memory(memory);

            let buffer = ManuallyDrop::into_inner(ptr::read(&self.buffer));
            self.device.destroy_buffer(buffer);
        }
    }
}
