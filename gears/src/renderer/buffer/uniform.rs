use ash::vk;
use std::sync::Arc;

use super::{stage::StageBuffer, Buffer, BufferError, WriteType};
use crate::renderer::{device::RenderDevice, Renderer, UpdateRecordInfo};

pub struct UniformBuffer<T> {
    stage: StageBuffer<T>, // the uniform buffer itself
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
        Ok(Self {
            stage: StageBuffer::new_with_usage(
                device.clone(),
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                1,
                true,
            )?,
        })
    }

    pub fn write(&mut self, data: &T) -> Result<WriteType, BufferError> {
        self.stage.write_single(0, data)
    }
}

impl<T> Buffer for UniformBuffer<T> {
    unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        self.stage.update(uri)
    }

    fn get(&self) -> vk::Buffer {
        self.stage.get()
    }
}
