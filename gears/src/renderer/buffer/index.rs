use std::{
    iter,
    mem::{self, ManuallyDrop},
    ptr,
    sync::Arc,
};

use gfx_hal::{
    adapter::MemoryType,
    buffer::{SubRange, Usage},
    command::CommandBuffer,
    device::Device,
    memory::{Properties, Segment},
    Backend, IndexType,
};

use crate::{FrameCommands, GearsRenderer};

use super::{upload_type, BufferError, VertexBuffer};

pub struct IndexBuffer<B: Backend> {
    device: Arc<B::Device>,

    buffer: ManuallyDrop<B::Buffer>,
    memory: ManuallyDrop<B::Memory>,

    // not bytes
    len: usize,
    capacity: usize,
}

impl<B: Backend> IndexBuffer<B> {
    pub fn new(renderer: &GearsRenderer<B>, size: usize) -> Result<Self, BufferError> {
        Self::new_with_device(renderer.device.clone(), &renderer.memory_types, size)
    }

    pub fn new_with_data(renderer: &GearsRenderer<B>, data: &[u32]) -> Result<Self, BufferError> {
        let mut buffer = Self::new(renderer, data.len())?;
        buffer.write(0, data)?;
        Ok(buffer)
    }

    pub fn new_with_device(
        device: Arc<B::Device>,
        available_memory_types: &Vec<MemoryType>,
        size: usize,
    ) -> Result<Self, BufferError> {
        if size == 0 {
            Err(BufferError::InvalidSize)
        } else {
            let byte_len = size * mem::size_of::<u32>();
            let mut buffer = ManuallyDrop::new(
                unsafe { device.create_buffer(byte_len as u64, Usage::INDEX) }
                    .or(Err(BufferError::OutOfMemory))?,
            );
            let req = unsafe { device.get_buffer_requirements(&buffer) };

            let memory = ManuallyDrop::new(
                unsafe {
                    device.allocate_memory(
                        upload_type(
                            available_memory_types,
                            &req,
                            Properties::CPU_VISIBLE | Properties::COHERENT,
                            Properties::CPU_VISIBLE,
                        ),
                        req.size,
                    )
                }
                .or(Err(BufferError::OutOfMemory))?,
            );
            unsafe { device.bind_buffer_memory(&memory, 0, &mut buffer) }
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
                // map
                let mapping = self
                    .device
                    .map_memory(&mut self.memory, Segment::ALL)
                    .unwrap();
                // write
                ptr::copy_nonoverlapping(
                    data.as_ptr() as *const u8,
                    mapping.add(mem::size_of::<u32>() * offset),
                    mem::size_of::<u32>() * data.len(),
                );
                // flush
                self.device
                    .flush_mapped_memory_ranges(iter::once((&*self.memory, Segment::ALL)))
                    .unwrap();
                // unmap
                self.device.unmap_memory(&mut self.memory);
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

    pub fn bind(&self, command_buffer: &mut B::CommandBuffer) {
        unsafe {
            command_buffer.bind_index_buffer(&self.buffer, SubRange::WHOLE, IndexType::U32);
        }
    }

    pub fn draw<T>(&self, vertices: &VertexBuffer<T, B>, command_buffer: &mut FrameCommands<B>) {
        self.bind(command_buffer);
        unsafe {
            vertices.bind(command_buffer);
            command_buffer.draw_indexed(0..(self.len() as u32), 0, 0..1);
        }
    }
}

impl<B: Backend> Drop for IndexBuffer<B> {
    fn drop(&mut self) {
        unsafe {
            let memory = ManuallyDrop::into_inner(ptr::read(&self.memory));
            self.device.free_memory(memory);

            let buffer = ManuallyDrop::into_inner(ptr::read(&self.buffer));
            self.device.destroy_buffer(buffer);
        }
    }
}
