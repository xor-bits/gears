use ash::{version::DeviceV1_0, vk};
use std::{marker::PhantomData, mem, ptr, sync::Arc};

use crate::Renderer;

use super::{upload_type, BufferError};

pub struct UniformBuffer<T> {
    device: Arc<ash::Device>,

    buffer: vk::Buffer,
    memory: vk::DeviceMemory,

    _p: PhantomData<T>,
}

impl<T> UniformBuffer<T> {
    pub fn new(renderer: &Renderer) -> Result<Self, BufferError> {
        Self::new_with_device(
            renderer.device.clone(),
            &renderer.memory_properties.memory_types,
        )
    }

    pub fn new_with_data(renderer: &Renderer, data: &T) -> Result<Self, BufferError> {
        let buffer = Self::new(renderer)?;
        buffer.write(data);
        Ok(buffer)
    }

    pub fn new_with_device(
        device: Arc<ash::Device>,
        available_memory_types: &[vk::MemoryType],
    ) -> Result<Self, BufferError> {
        let byte_len = mem::size_of::<T>();

        let buffer_info = vk::BufferCreateInfo::builder()
            .size(byte_len as u64)
            .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
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

            _p: PhantomData::default(),
        })
    }

    pub fn write(&self, data: &T) {
        let data_size = mem::size_of::<T>();

        unsafe {
            // map
            let mapping = self
                .device
                .map_memory(self.memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
                .unwrap();
            // write
            ptr::copy_nonoverlapping(data as *const T as *const u8, mapping as *mut u8, data_size);
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
    }

    pub fn get(&self) -> vk::Buffer {
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
