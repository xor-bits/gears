use super::{create_buffer, stage::StageBuffer, BufferError};
use crate::{
    renderer::{device::Dev, RenderRecordInfo, Renderer, UpdateRecordInfo},
    Buffer, MultiWriteBuffer, WriteType,
};
use ash::{version::DeviceV1_0, vk};
use log::debug;
use std::mem;

pub trait UInt: PartialEq {
    fn get() -> vk::IndexType;
}

impl UInt for u16 {
    fn get() -> vk::IndexType {
        vk::IndexType::UINT16
    }
}
impl UInt for u32 {
    fn get() -> vk::IndexType {
        vk::IndexType::UINT32
    }
}

pub struct IndexBuffer<I>
where
    I: UInt,
{
    device: Dev,

    buffer: vk::Buffer,
    memory: vk::DeviceMemory,

    requested_copy: bool,
    stage: StageBuffer<I>,
}

impl<I> IndexBuffer<I>
where
    I: UInt,
{
    pub fn new(renderer: &Renderer, size: usize) -> Result<Self, BufferError> {
        Self::new_with_device(renderer.rdevice.clone(), size)
    }

    pub fn new_with_data(renderer: &Renderer, data: &[I]) -> Result<Self, BufferError> {
        let mut buffer = Self::new(renderer, data.len())?;
        buffer.write(0, data)?;
        Ok(buffer)
    }

    pub fn new_with_device(device: Dev, size: usize) -> Result<Self, BufferError> {
        let byte_len = size * mem::size_of::<I>();
        let (buffer, memory) = create_buffer(
            &device,
            byte_len,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
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
        if rri.debug_calls {
            debug!("cmd_bind_index_buffer");
        }

        self.device
            .cmd_bind_index_buffer(rri.command_buffer, self.buffer, 0, I::get());
    }
}

impl<I> MultiWriteBuffer<I> for IndexBuffer<I>
where
    I: UInt,
{
    fn write(&mut self, offset: usize, data: &[I]) -> Result<WriteType, BufferError> {
        let result = self.stage.write_slice(offset, data);
        self.requested_copy = result == Ok(WriteType::Write) || self.requested_copy;
        result
    }
}

impl<I> Buffer<I> for IndexBuffer<I>
where
    I: UInt,
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

impl<I> Drop for IndexBuffer<I>
where
    I: UInt,
{
    fn drop(&mut self) {
        unsafe {
            self.device.free_memory(self.memory, None);
            self.device.destroy_buffer(self.buffer, None);
        }
    }
}
