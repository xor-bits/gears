use ash::{version::DeviceV1_0, vk};
use std::{
    mem,
    sync::atomic::{AtomicBool, Ordering},
};

use log::debug;

use crate::renderer::{device::Dev, RenderRecordInfo, Renderer, UpdateRecordInfo};

use super::{create_buffer, stage::StageBuffer, Buffer, BufferError, WriteType};

pub struct VertexBuffer<T> {
    device: Dev,

    buffer: vk::Buffer,
    memory: vk::DeviceMemory,

    requested_copy: AtomicBool,
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

            requested_copy: AtomicBool::new(false),
            stage,
        })
    }

    pub fn write(&mut self, offset: usize, data: &[T]) -> Result<WriteType, BufferError> {
        let result = self.stage.write_slice(offset, data);
        if let Ok(WriteType::Write) = result {
            self.requested_copy.store(true, Ordering::SeqCst);
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

        if rri.debug_calls {
            debug!("cmd_bind_vertex_buffers");
        }

        self.device
            .cmd_bind_vertex_buffers(rri.command_buffer, 0, &buffer, &offsets);
    }

    pub unsafe fn draw(&self, rri: &RenderRecordInfo) {
        self.bind(rri);

        rri.triangles.fetch_add(self.len() / 3, Ordering::SeqCst);

        if rri.debug_calls {
            debug!("cmd_draw");
        }

        self.device
            .cmd_draw(rri.command_buffer, self.len() as u32, 1, 0, 0);
    }
}

impl<T> Buffer for VertexBuffer<T> {
    unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        let requested_copy = self.requested_copy.swap(false, Ordering::SeqCst);

        if requested_copy {
            self.stage.copy_to(uri, self);
        }

        requested_copy
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
