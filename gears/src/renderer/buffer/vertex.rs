use super::{create_buffer, stage::StageBuffer, Buffer, BufferError, WriteType};
use crate::{
    renderer::{device::Dev, RenderRecordInfo, Renderer, UpdateRecordInfo},
    MultiWriteBuffer,
};
use ash::{version::DeviceV1_0, vk};
use log::debug;
use std::mem;

pub struct VertexBuffer<T>
where
    T: PartialEq,
{
    device: Dev,

    buffer: vk::Buffer,
    memory: vk::DeviceMemory,

    requested_copy: bool,
    stage: StageBuffer<T>,
}

impl<T> VertexBuffer<T>
where
    T: PartialEq,
{
    pub fn new(renderer: &Renderer, size: usize) -> Result<Self, BufferError> {
        Self::new_with_device(renderer.rdevice.clone(), size)
    }

    pub fn new_with_data(renderer: &Renderer, data: &[T]) -> Result<Self, BufferError> {
        let mut buffer = Self::new(renderer, data.len())?;
        buffer.write(0, data)?;
        Ok(buffer)
    }

    pub fn new_with_device(device: Dev, size: usize) -> Result<Self, BufferError> {
        let byte_len = size * mem::size_of::<T>();
        let (buffer, memory) = create_buffer(
            &device,
            byte_len,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        let stage = StageBuffer::new_with_device(device.clone(), size, true)?;

        Ok(Self {
            device,

            buffer,
            memory,

            requested_copy: false,
            stage,
        })
    }

    pub unsafe fn bind(&self, rri: &RenderRecordInfo) {
        let buffer = [self.buffer];
        let offsets = [0];

        if rri.debug_calls {
            debug!("cmd_bind_vertex_buffers");
        }

        self.device
            .cmd_bind_vertex_buffers(rri.command_buffer, 0, &buffer, &offsets);
    }
}

impl<T> MultiWriteBuffer<T> for VertexBuffer<T>
where
    T: PartialEq,
{
    fn write(&mut self, offset: usize, data: &[T]) -> Result<WriteType, BufferError> {
        let result = self.stage.write_slice(offset, data);
        self.requested_copy = result == Ok(WriteType::Write) || self.requested_copy;
        result
    }
}

impl<T> Buffer<T> for VertexBuffer<T>
where
    T: PartialEq,
{
    unsafe fn update(&mut self, uri: &UpdateRecordInfo) -> bool {
        let req = self.requested_copy;
        if req {
            self.requested_copy = false;
            self.stage.copy_to(uri, self);
        }
        req
    }

    fn buffer(&self) -> vk::Buffer {
        self.buffer
    }

    fn len(&self) -> usize {
        self.stage.len()
    }

    fn capacity(&self) -> usize {
        self.stage.capacity()
    }
}

impl<T> Drop for VertexBuffer<T>
where
    T: PartialEq,
{
    fn drop(&mut self) {
        unsafe {
            self.device.free_memory(self.memory, None);
            self.device.destroy_buffer(self.buffer, None);
        }
    }
}
