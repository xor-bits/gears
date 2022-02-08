use std::marker::PhantomData;

use crate::Dev;
use ash::{version::DeviceV1_0, vk};

pub struct SimpleBuffer<T> {
    pub device: Dev,
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,

    // not bytes
    pub capacity: usize,

    _p: PhantomData<T>,
}

impl<T> SimpleBuffer<T> {
    pub fn new(device: Dev, buffer: vk::Buffer, memory: vk::DeviceMemory, capacity: usize) -> Self {
        Self {
            device,
            buffer,
            memory,
            capacity,

            _p: PhantomData {},
        }
    }
}

impl<T> Drop for SimpleBuffer<T> {
    fn drop(&mut self) {
        log::debug!("Dropping SimpleBuffer {:?}", self.memory);
        unsafe {
            self.device.free_memory(self.memory, None);
            self.device.destroy_buffer(self.buffer, None)
        }
    }
}
