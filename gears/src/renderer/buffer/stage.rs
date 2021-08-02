use super::{create_buffer_with_fallback, Buffer, BufferError, WriteType};
use crate::renderer::{device::Dev, UpdateRecordInfo};
use ash::{version::DeviceV1_0, vk};
use core::slice;
use std::{marker::PhantomData, mem};

pub struct MemMapHandle<'a, T> {
    ptr: *mut T,
    ptr_len: usize, // elements not bytes

    device: &'a Dev,
    memory: vk::DeviceMemory,
    non_coherent: bool,
}

pub struct StageBuffer<T>
where
    T: PartialEq,
{
    device: Dev,

    buffer: vk::Buffer,
    memory: vk::DeviceMemory,

    // not bytes
    len: usize,
    capacity: usize,

    non_coherent: bool,
    write_optimize: bool,

    _p: PhantomData<T>,
}

impl<T> StageBuffer<T>
where
    T: PartialEq,
{
    pub fn new_with_device(
        device: Dev,
        size: usize,
        write_optimize: bool,
    ) -> Result<Self, BufferError> {
        Self::new_with_usage(
            device,
            vk::BufferUsageFlags::TRANSFER_SRC,
            size,
            write_optimize,
        )
    }

    pub fn new_with_usage(
        device: Dev,
        usage: vk::BufferUsageFlags,
        size: usize,
        write_optimize: bool,
    ) -> Result<Self, BufferError> {
        let byte_len = size * mem::size_of::<T>();
        let (buffer, memory, non_coherent) = create_buffer_with_fallback(
            &device,
            byte_len,
            usage,
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
            write_optimize,

            _p: PhantomData::default(),
        })
    }

    pub unsafe fn map_buffer(
        &mut self,
        element_count: usize,
        element_offset: usize,
    ) -> Result<MemMapHandle<'_, T>, BufferError> {
        let len = element_offset + element_count;
        let cap = self.capacity;
        if len > cap {
            Err(BufferError::TriedToOverflow)
        } else {
            self.len = self.len.max(len);

            let size = self.capacity() - element_offset;
            let mem_size = (mem::size_of::<T>() * size) as u64;
            let mem_offset = (mem::size_of::<T>() * element_offset) as u64;

            let mapping = self
                .device
                .map_memory(
                    self.memory,
                    mem_offset,
                    mem_size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap() as *mut T;

            Ok(MemMapHandle {
                ptr: mapping,
                ptr_len: element_count,

                device: &self.device,
                memory: self.memory,
                non_coherent: self.non_coherent,
            })
        }
    }

    pub unsafe fn write_bytes(
        &mut self,
        element_offset: usize,
        elements: &[T],
    ) -> Result<WriteType, BufferError> {
        let opt = self.write_optimize;
        let map_handle = self.map_buffer(elements.len(), element_offset)?;

        if opt && slice::from_raw_parts(map_handle.ptr, elements.len()) == elements {
            Ok(WriteType::NoWrite)
        } else {
            map_handle.copy_from(elements)?;
            Ok(WriteType::Write)
        }
    }

    pub fn write_slice(&mut self, offset: usize, data: &[T]) -> Result<WriteType, BufferError> {
        unsafe { self.write_bytes(offset, data) }
    }

    pub fn write_single(&mut self, offset: usize, data: &T) -> Result<WriteType, BufferError> {
        unsafe { self.write_bytes(offset, slice::from_ref(data)) }
    }

    pub unsafe fn copy_to(&self, uri: &UpdateRecordInfo, dst: &dyn Buffer<T>) {
        // TODO: only copy modified regions
        let regions = [vk::BufferCopy::builder()
            .src_offset(0)
            .dst_offset(0)
            .size((mem::size_of::<T>() * self.capacity()) as u64)
            .build()];

        self.device
            .cmd_copy_buffer(uri.command_buffer, self.buffer, dst.buffer(), &regions);
    }
}

impl<'a, T> MemMapHandle<'a, T> {
    pub unsafe fn copy_from(&self, elements: &[T]) -> Result<(), BufferError> {
        if elements.len() > self.ptr_len {
            Err(BufferError::TriedToOverflow)
        } else {
            elements
                .as_ptr()
                .copy_to_nonoverlapping(self.ptr, elements.len());
            Ok(())
        }
    }
}

impl<'a, T> Drop for MemMapHandle<'a, T> {
    fn drop(&mut self) {
        if self.non_coherent {
            // flush
            let ranges = [vk::MappedMemoryRange::builder()
                .memory(self.memory)
                .offset(0)
                .size(vk::WHOLE_SIZE)
                .build()];

            unsafe { self.device.flush_mapped_memory_ranges(&ranges) }.unwrap();
        }
        // unmap
        unsafe { self.device.unmap_memory(self.memory) };
    }
}

impl<T> Buffer<T> for StageBuffer<T>
where
    T: PartialEq,
{
    unsafe fn update(&mut self, _: &UpdateRecordInfo) -> bool {
        false
    }

    fn buffer(&self) -> vk::Buffer {
        self.buffer
    }

    fn len(&self) -> usize {
        self.len
    }

    fn capacity(&self) -> usize {
        self.capacity
    }
}
