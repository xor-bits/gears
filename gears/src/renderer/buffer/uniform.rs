use super::{stage::StageBuffer, Buffer, BufferError, WriteType};
use crate::{
    renderer::{device::Dev, Renderer, UpdateRecordInfo},
    WriteBuffer,
};
use ash::vk;

pub struct UniformBuffer<T>
where
    T: PartialEq,
{
    stage: StageBuffer<T>, // the uniform buffer itself
}

impl<T> UniformBuffer<T>
where
    T: PartialEq,
{
    pub fn new(renderer: &Renderer) -> Result<Self, BufferError> {
        Self::new_with_device(renderer.rdevice.clone())
    }

    pub fn new_with_data(renderer: &Renderer, data: &T) -> Result<Self, BufferError> {
        let mut buffer = Self::new(renderer)?;
        buffer.write(data)?;
        Ok(buffer)
    }

    pub fn new_with_device(device: Dev) -> Result<Self, BufferError> {
        Ok(Self {
            stage: StageBuffer::new_with_usage(
                device.clone(),
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                1,
                true,
            )?,
        })
    }
}

impl<T> WriteBuffer<T> for UniformBuffer<T>
where
    T: PartialEq,
{
    fn write(&mut self, data: &T) -> Result<WriteType, BufferError> {
        self.stage.write_single(0, data)
    }
}

impl<T> Buffer<T> for UniformBuffer<T>
where
    T: PartialEq,
{
    unsafe fn update(&mut self, uri: &UpdateRecordInfo) -> bool {
        self.stage.update(uri)
    }

    fn buffer(&self) -> vk::Buffer {
        self.stage.buffer()
    }

    fn len(&self) -> usize {
        self.stage.len()
    }

    fn capacity(&self) -> usize {
        self.stage.capacity()
    }
}
