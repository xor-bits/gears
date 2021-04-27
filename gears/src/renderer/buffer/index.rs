use ash::{version::DeviceV1_0, vk};
use std::{mem, ptr, sync::Arc};

use crate::{RenderRecordInfo, Renderer};

use super::{upload_type, BufferError, VertexBuffer};

pub struct IndexBuffer {
    device: Arc<ash::Device>,

    buffer: vk::Buffer,
    memory: vk::DeviceMemory,

    // not bytes
    len: usize,
    capacity: usize,
}

impl IndexBuffer {
    pub fn new(renderer: &Renderer, size: usize) -> Result<Self, BufferError> {
        Self::new_with_device(
            renderer.device.clone(),
            &renderer.memory_properties.memory_types,
            size,
        )
    }

    pub fn new_with_data(renderer: &Renderer, data: &[u32]) -> Result<Self, BufferError> {
        let mut buffer = Self::new(renderer, data.len())?;
        buffer.write(0, data)?;
        Ok(buffer)
    }

    pub fn new_with_device(
        device: Arc<ash::Device>,
        available_memory_types: &[vk::MemoryType],
        size: usize,
    ) -> Result<Self, BufferError> {
        if size == 0 {
            Err(BufferError::InvalidSize)
        } else {
            let byte_len = size * mem::size_of::<u32>();

            let buffer_info = vk::BufferCreateInfo::builder()
                .size(byte_len as u64)
                .usage(vk::BufferUsageFlags::INDEX_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let buffer = unsafe { device.create_buffer(&buffer_info, None) }
                .or(Err(BufferError::OutOfMemory))?;

            let req = unsafe { device.get_buffer_memory_requirements(buffer) };

            let alloc_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(req.size)
                .memory_type_index(upload_type(
                    available_memory_types,
                    &req,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                    vk::MemoryPropertyFlags::HOST_VISIBLE,
                ));

            let memory = unsafe { device.allocate_memory(&alloc_info, None) }
                .or(Err(BufferError::OutOfMemory))?;

            unsafe { device.bind_buffer_memory(buffer, memory, 0) }
                .or(Err(BufferError::OutOfMemory))?;

            Ok(Self {
                device,
                buffer,
                memory,
                len: 0,
                capacity: size,
            })
        }
    }

    pub fn write(&mut self, offset: usize, data: &[u32]) -> Result<(), BufferError> {
        self.len = offset + data.len();
        if self.len > self.capacity {
            Err(BufferError::TriedToOverflow)
        } else {
            unsafe {
                let memory_offset = mem::size_of::<u32>() * offset;
                let memory_size = mem::size_of::<u32>() * data.len();

                // map
                let mapping = self
                    .device
                    .map_memory(self.memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
                    .unwrap();
                // write
                ptr::copy_nonoverlapping(
                    (data.as_ptr() as *const u8).add(memory_offset),
                    mapping as *mut u8,
                    memory_size,
                );
                // flush
                self.device
                    .flush_mapped_memory_ranges(&[vk::MappedMemoryRange::builder()
                        .memory(self.memory)
                        .offset(0)
                        .size(vk::WHOLE_SIZE)
                        .build()])
                    .unwrap();
                // unmap
                self.device.unmap_memory(self.memory);
            }
            Ok(())
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn bind(&self, rri: &RenderRecordInfo) {
        unsafe {
            self.device.cmd_bind_index_buffer(
                rri.command_buffer,
                self.buffer,
                0,
                vk::IndexType::UINT32,
            );
        }
    }

    pub fn draw<T>(&self, rri: &RenderRecordInfo, vertices: &VertexBuffer<T>) {
        self.bind(rri);
        vertices.bind(rri);

        unsafe {
            self.device
                .cmd_draw_indexed(rri.command_buffer, self.len() as u32, 1, 0, 0, 0);
        }
    }
}

impl Drop for IndexBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.free_memory(self.memory, None);
            self.device.destroy_buffer(self.buffer, None);
        }
    }
}
