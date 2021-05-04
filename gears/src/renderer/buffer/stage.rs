use std::{
    collections::hash_map::DefaultHasher, hash::Hasher, marker::PhantomData, mem, sync::Arc,
};

use ash::{version::DeviceV1_0, vk};

use crate::{renderer::device::RenderDevice, Buffer, BufferError, UpdateRecordInfo};

use super::create_buffer_with_fallback;

pub enum WriteType {
    NoWrite,
    Write,
}

pub struct StageBuffer<T> {
    device: Arc<RenderDevice>,

    buffer: vk::Buffer,
    memory: vk::DeviceMemory,

    // not bytes
    len: usize,
    capacity: usize,

    non_coherent: bool,
    last_hash: u64,
    write_optimize: bool,

    _p: PhantomData<T>,
}

impl<T> StageBuffer<T> {
    pub fn new_with_device(
        device: Arc<RenderDevice>,
        size: usize,
        write_optimize: bool,
    ) -> Result<Self, BufferError> {
        let byte_len = size * mem::size_of::<T>();
        let (buffer, memory, non_coherent) = create_buffer_with_fallback(
            &device,
            byte_len,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;

        Ok(Self {
            device,

            buffer,
            memory,

            len: 0,
            capacity: size,

            non_coherent,
            last_hash: 0,
            write_optimize,

            _p: PhantomData::default(),
        })
    }

    pub unsafe fn write_bytes(
        &mut self,
        bytes_in_cpu: *const u8,
        count_in_cpu: usize,
        offset_in_gpu: usize,
    ) -> Result<WriteType, BufferError> {
        self.len = offset_in_gpu + count_in_cpu;
        if self.len > self.capacity {
            Err(BufferError::TriedToOverflow)
        } else {
            let memory_size = mem::size_of::<T>() * count_in_cpu;

            if self.write_optimize {
                let mut hasher = DefaultHasher::new();
                hash_from_ptr(&mut hasher, bytes_in_cpu, memory_size);
                let new_hash = hasher.finish();

                if self.last_hash == new_hash {
                    return Ok(WriteType::NoWrite);
                }
                self.last_hash = new_hash;
            }

            // map
            let mapping = self
                .device
                .map_memory(
                    self.memory,
                    offset_in_gpu as u64,
                    vk::WHOLE_SIZE,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap() as *mut u8;
            // write
            bytes_in_cpu.copy_to_nonoverlapping(mapping, memory_size);
            if self.non_coherent {
                // flush
                let ranges = [vk::MappedMemoryRange::builder()
                    .memory(self.memory)
                    .offset(0)
                    .size(vk::WHOLE_SIZE)
                    .build()];
                self.device.flush_mapped_memory_ranges(&ranges).unwrap();
            }
            // unmap
            self.device.unmap_memory(self.memory);
            Ok(WriteType::Write)
        }
    }

    pub fn write_slice(&mut self, offset: usize, data: &[T]) -> Result<WriteType, BufferError> {
        unsafe { self.write_bytes(data.as_ptr() as *const u8, data.len(), offset) }
    }

    pub fn write_single(&mut self, offset: usize, data: &T) -> Result<WriteType, BufferError> {
        unsafe { self.write_bytes(data as *const T as *const u8, 1, offset) }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub unsafe fn copy_to(&self, uri: &UpdateRecordInfo, dst: &dyn Buffer) {
        let memory_size = mem::size_of::<T>() * self.len;

        // TODO: only copy modified regions
        let regions = [vk::BufferCopy::builder()
            .src_offset(0)
            .dst_offset(0)
            .size(memory_size as u64)
            .build()];
        self.device
            .cmd_copy_buffer(uri.command_buffer, self.buffer, dst.get(), &regions);
    }
}

unsafe fn hash_from_ptr(hasher: &mut dyn Hasher, ptr: *const u8, len: usize) {
    for i in 0..len {
        hasher.write_u8(*ptr.add(i));
    }
}
