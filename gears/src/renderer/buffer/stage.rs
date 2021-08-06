use super::{create_buffer_with_fallback, Buffer, BufferError, WriteType};
use crate::{
    renderer::{device::Dev, UpdateRecordInfo},
    ReAllocatableBuffer, SimpleBuffer,
};
use ash::{version::DeviceV1_0, vk};
use core::slice;
use std::mem;

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
    usage: vk::BufferUsageFlags,
    non_coherent: bool,
    buffer: ReAllocatableBuffer<T>,
}

impl<T> StageBuffer<T>
where
    T: PartialEq,
{
    fn current(&self) -> &'_ SimpleBuffer<T> {
        &self.buffer.buffer
    }

    pub fn new_with_device(device: Dev, size: usize) -> Result<Self, BufferError> {
        Self::new_with_usage(device, vk::BufferUsageFlags::TRANSFER_SRC, size)
    }

    pub fn new_with_usage(
        device: Dev,
        usage: vk::BufferUsageFlags,
        size: usize,
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

        let buffer = ReAllocatableBuffer::new(SimpleBuffer::new(device, buffer, memory, size));

        Ok(Self {
            buffer,
            usage,
            non_coherent,
        })
    }

    pub fn re_alloc(&mut self, new_size: usize) -> Result<(), BufferError> {
        let byte_len = new_size * mem::size_of::<T>();
        let (buffer, memory, _) = create_buffer_with_fallback(
            &self.current().device,
            byte_len,
            self.usage,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            vk::MemoryPropertyFlags::HOST_VISIBLE,
        )?;

        let device = self.current().device.clone();

        Ok(self
            .buffer
            .re_alloc(SimpleBuffer::new(device, buffer, memory, new_size)))
    }

    pub unsafe fn map_buffer(
        &mut self,
        element_count: usize,
        element_offset: usize,
    ) -> Result<(WriteType, MemMapHandle<'_, T>), BufferError> {
        // re alloc if it would overflow
        let len = element_offset + element_count;
        let cap = self.current().capacity;
        let write_type = if len > cap {
            self.re_alloc(len)?;
            WriteType::Resize
        } else {
            WriteType::Write
        };

        // pre calc
        let size = self.elem_capacity() - element_offset;
        let mem_size = (mem::size_of::<T>() * size) as u64;
        let mem_offset = (mem::size_of::<T>() * element_offset) as u64;

        // map
        let mapping = self
            .current()
            .device
            .map_memory(
                self.current().memory,
                mem_offset,
                mem_size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap() as *mut T;

        Ok((
            write_type,
            MemMapHandle {
                ptr: mapping,
                ptr_len: element_count,

                device: &self.current().device,
                memory: self.current().memory,
                non_coherent: self.non_coherent,
            },
        ))
    }

    pub unsafe fn write_bytes(
        &mut self,
        element_offset: usize,
        elements: &[T],
    ) -> Result<WriteType, BufferError> {
        let (write_type, map_handle) = self.map_buffer(elements.len(), element_offset)?;

        if slice::from_raw_parts(map_handle.ptr, elements.len()) == elements {
            Ok(WriteType::NoWrite)
        } else {
            map_handle.copy_from(elements)?;
            Ok(write_type)
        }
    }

    pub fn write_slice(&mut self, offset: usize, data: &[T]) -> Result<WriteType, BufferError> {
        unsafe { self.write_bytes(offset, data) }
    }

    pub fn write_single(&mut self, offset: usize, data: &T) -> Result<WriteType, BufferError> {
        unsafe { self.write_bytes(offset, slice::from_ref(data)) }
    }

    pub unsafe fn copy_to_raw(&self, uri: &UpdateRecordInfo, dst: vk::Buffer, size: u64) {
        // TODO: only copy modified regions
        let regions = [vk::BufferCopy::builder()
            .src_offset(0)
            .dst_offset(0)
            .size(size)
            .build()];

        self.current().device.cmd_copy_buffer(
            uri.command_buffer,
            self.current().buffer,
            dst,
            &regions,
        );
    }

    pub unsafe fn copy_to(&self, uri: &UpdateRecordInfo, dst: &dyn Buffer<T>) {
        self.copy_to_raw(
            uri,
            dst.buffer(),
            self.byte_capacity().min(dst.byte_capacity()) as u64,
        )
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
        // TODO: copy old buffers to the reallocated one
        /* let c_cap = self.current().capacity;
        let c_buffer = self.current().buffer;
        let device = self.current().device.clone();

        let updates = self.buffer.re_allocates.iter().peekable().peek().is_some();

        let iter = self.buffer.re_allocates.iter_mut().filter_map(|re_alloc| {
            if let Some((update, buffer)) = re_alloc.as_mut() {
                if *update {
                    *update = false;
                    Some(buffer)
                } else {
                    None
                }
            } else {
                None
            }
        });

        for update in iter {
            let size = c_cap.min(update.capacity) as u64;

            let regions = [vk::BufferCopy::builder()
                .src_offset(0)
                .dst_offset(0)
                .size(size)
                .build()];

            device.cmd_copy_buffer(uri.command_buffer, update.buffer, c_buffer, &regions);
        }

        updates */
        false
    }

    fn buffer(&self) -> vk::Buffer {
        self.current().buffer
    }

    fn elem_capacity(&self) -> usize {
        self.current().capacity
    }
}
