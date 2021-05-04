use ash::{version::DeviceV1_0, vk};
use std::{mem, sync::Arc};

use crate::{
    renderer::device::RenderDevice, Buffer, RenderRecordInfo, Renderer, StageBuffer, UpdateQuery,
    UpdateRecordInfo, WriteType,
};

use super::{create_buffer, BufferError};

pub struct VertexBuffer<T> {
    device: Arc<RenderDevice>,

    buffer: vk::Buffer,
    memory: vk::DeviceMemory,

    requested_copy: bool,
    stage: StageBuffer<T>,
}

impl<T> VertexBuffer<T> {
    pub fn new(renderer: &Renderer, size: usize) -> Result<Self, BufferError> {
        Self::new_with_device(renderer.rdevice.clone(), size)
    }

    pub fn new_with_data(renderer: &Renderer, data: &[T]) -> Result<Self, BufferError> {
        let mut buffer = Self::new(renderer, data.len())?;
        buffer.write(0, data)?;
        Ok(buffer)
    }

    pub fn new_with_device(device: Arc<RenderDevice>, size: usize) -> Result<Self, BufferError> {
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

    pub fn write(&mut self, offset: usize, data: &[T]) -> Result<WriteType, BufferError> {
        let result = self.stage.write_slice(offset, data);
        if let Ok(WriteType::Write) = result {
            self.requested_copy = true
        }
        result
    }

    pub fn len(&self) -> usize {
        self.stage.len()
    }

    pub fn capacity(&self) -> usize {
        self.stage.capacity()
    }

    pub unsafe fn bind(&self, rri: &RenderRecordInfo) {
        let buffer = [self.buffer];
        let offsets = [0];

        self.device
            .cmd_bind_vertex_buffers(rri.command_buffer, 0, &buffer, &offsets);
    }

    pub unsafe fn draw(&self, rri: &RenderRecordInfo) {
        self.bind(rri);

        self.device
            .cmd_draw(rri.command_buffer, self.len() as u32, 1, 0, 0);
    }
}

impl<T> Buffer for VertexBuffer<T> {
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

impl<T> Drop for VertexBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            self.device.free_memory(self.memory, None);
            self.device.destroy_buffer(self.buffer, None);
        }
    }
}
