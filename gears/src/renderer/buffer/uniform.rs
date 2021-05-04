use ash::{version::DeviceV1_0, vk};
use std::{mem, sync::Arc};

use crate::renderer::{device::RenderDevice, Renderer, UpdateQuery, UpdateRecordInfo};
use super::{create_buffer, stage::StageBuffer, Buffer, BufferError, WriteType};

pub struct UniformBuffer<T> {
    device: Arc<RenderDevice>,

    buffer: vk::Buffer,
    memory: vk::DeviceMemory,

    requested_copy: bool,
    stage: StageBuffer<T>,
}

impl<T> UniformBuffer<T> {
    pub fn new(renderer: &Renderer) -> Result<Self, BufferError> {
        Self::new_with_device(renderer.rdevice.clone())
    }

    pub fn new_with_data(renderer: &Renderer, data: &T) -> Result<Self, BufferError> {
        let mut buffer = Self::new(renderer)?;
        buffer.write(data)?;
        Ok(buffer)
    }

    pub fn new_with_device(device: Arc<RenderDevice>) -> Result<Self, BufferError> {
        let byte_len = mem::size_of::<T>();
        let (buffer, memory) = create_buffer(
            &device,
            byte_len,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        let stage = StageBuffer::new_with_device(device.clone(), 1, true)?;

        Ok(Self {
            device,

            buffer,
            memory,

            requested_copy: false,
            stage,
        })
    }

    pub fn write(&mut self, data: &T) -> Result<WriteType, BufferError> {
        let result = self.stage.write_single(0, data);
        if let Ok(WriteType::Write) = result {
            self.requested_copy = true
        }
        result
    }
}

impl<T> Buffer for UniformBuffer<T> {
    fn updates(&self, _: &UpdateQuery) -> bool {
        self.requested_copy
    }

    unsafe fn update(&mut self, uri: &UpdateRecordInfo) {
        if self.requested_copy {
            self.requested_copy = false;
            self.stage.copy_to(uri, self);
        }
    }

    fn get(&self) -> vk::Buffer {
        self.buffer
    }
}

impl<T> Drop for UniformBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            self.device.free_memory(self.memory, None);
            self.device.destroy_buffer(self.buffer, None);
        }
    }
}
