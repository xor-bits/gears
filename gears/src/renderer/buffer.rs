pub mod debug;
pub mod image;
pub mod index;
pub mod indirect;
pub mod re_alloc;
pub mod simple;
pub mod stage;
pub mod uniform;
pub mod vertex;

#[cfg(feature = "short_namespaces")]
pub use debug::*;
#[cfg(feature = "short_namespaces")]
pub use image::*;
#[cfg(feature = "short_namespaces")]
pub use index::*;
#[cfg(feature = "short_namespaces")]
pub use indirect::*;
#[cfg(feature = "short_namespaces")]
pub use re_alloc::*;
#[cfg(feature = "short_namespaces")]
pub use simple::*;
#[cfg(feature = "short_namespaces")]
pub use stage::*;
#[cfg(feature = "short_namespaces")]
pub use uniform::*;
#[cfg(feature = "short_namespaces")]
pub use vertex::*;

use super::{device::Dev, UpdateRecordInfo};
use crate::DerefDev;
use ash::{version::DeviceV1_0, vk};
use std::mem;

#[derive(Debug, PartialEq, Eq)]
pub enum WriteType {
    NoWrite,
    Write,
    Resize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BufferError {
    NoUBOs,
    InvalidSize,
    TriedToOverflow,
    OutOfMemory,
    NoMemoryType(vk::MemoryPropertyFlags),
}

// buffer traits

pub trait MultiWriteBuffer<T>: Buffer<T> {
    fn write(&mut self, offset: usize, data: &[T]) -> Result<WriteType, BufferError>;
}

pub trait WriteBuffer<T>: Buffer<T> {
    fn write(&mut self, data: &T) -> Result<WriteType, BufferError>;
}

pub trait Buffer<T> {
    unsafe fn update(&mut self, uri: &UpdateRecordInfo) -> bool;

    fn buffer(&self) -> vk::Buffer;
    fn elem_capacity(&self) -> usize;
    fn byte_capacity(&self) -> usize {
        self.elem_capacity() * mem::size_of::<T>()
    }
}

// generic staged buffer

pub struct GenericStagedBuffer<
    T,
    const USAGE: u32, /* vk::BufferUsageFlags */
    const MULTI: bool,
> where
    T: PartialEq,
{
    requested_copy: bool,

    buffer: ReAllocatableBuffer<T>,
    stage: StageBuffer<T>,
}

impl<T, const USAGE: u32, const MULTI: bool> GenericStagedBuffer<T, USAGE, MULTI>
where
    T: PartialEq,
{
    fn current(&self) -> &'_ SimpleBuffer<T> {
        &self.buffer.buffer
    }

    fn usage() -> vk::BufferUsageFlags {
        vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::from_raw(USAGE)
    }

    fn new_with_device(device: Dev, size: usize) -> Result<Self, BufferError> {
        let byte_len = size * mem::size_of::<T>();
        let usage = Self::usage();
        let (buffer, memory) = create_buffer(
            &device,
            byte_len,
            usage,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        let buffer =
            ReAllocatableBuffer::new(SimpleBuffer::new(device.clone(), buffer, memory, size));

        let stage = StageBuffer::new_with_device(device, size)?;

        Ok(Self {
            requested_copy: false,
            buffer,
            stage,
        })
    }
}

// single data

impl<T, const USAGE: u32> GenericStagedBuffer<T, USAGE, false>
where
    T: PartialEq,
{
    pub fn new_single<D>(device: &D) -> Result<Self, BufferError>
    where
        D: DerefDev,
    {
        Self::new_with_device(device.deref_dev().clone(), 1)
    }

    pub fn new_with<D>(device: &D, data: &T) -> Result<Self, BufferError>
    where
        D: DerefDev,
    {
        let mut buffer = Self::new_single(device)?;
        buffer.write(data)?;
        Ok(buffer)
    }
}

impl<T, const USAGE: u32> WriteBuffer<T> for GenericStagedBuffer<T, USAGE, false>
where
    T: PartialEq,
{
    fn write(&mut self, data: &T) -> Result<WriteType, BufferError> {
        let result = self.stage.write_single(0, data);
        assert!(
            result != Ok(WriteType::Resize),
            "Single T wide buffer could not have been resized"
        );
        self.requested_copy = result == Ok(WriteType::Write) || self.requested_copy;
        result
    }
}

// multi data

impl<T, const USAGE: u32> GenericStagedBuffer<T, USAGE, true>
where
    T: PartialEq,
{
    pub fn new<D>(device: &D, count: usize) -> Result<Self, BufferError>
    where
        D: DerefDev,
    {
        Self::new_with_device(device.deref_dev().clone(), count)
    }

    pub fn new_with<D>(device: &D, data: &[T]) -> Result<Self, BufferError>
    where
        D: DerefDev,
    {
        let mut buffer = Self::new(device, data.len())?;
        buffer.write(0, data)?;
        Ok(buffer)
    }
}

impl<T, const USAGE: u32> MultiWriteBuffer<T> for GenericStagedBuffer<T, USAGE, true>
where
    T: PartialEq,
{
    fn write(&mut self, offset: usize, data: &[T]) -> Result<WriteType, BufferError> {
        let result = self.stage.write_slice(offset, data);

        if result == Ok(WriteType::Resize) {
            let len = offset + data.len();
            let usage = Self::usage();
            let device = self.current().device.clone();
            let (buffer, memory) = create_buffer(
                &device,
                len * mem::size_of::<T>(),
                usage,
                vk::SharingMode::EXCLUSIVE,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;

            self.buffer
                .re_alloc(SimpleBuffer::new(device, buffer, memory, len));
        }

        self.requested_copy = result != Ok(WriteType::NoWrite) || self.requested_copy;
        result
    }

    /* fn realloc(&mut self, new_size: usize) -> Result<(), BufferError> {
        let byte_len = new_size * mem::size_of::<T>();
        let (buffer, memory) = create_buffer(
            &self.device,
            byte_len,
            self.usage,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        self.stage.realloc(new_size)?;
        self.realloc = Some((buffer, memory));

        Ok(())
    } */
}

// common impl

impl<T, const USAGE: u32, const MULTI: bool> Buffer<T> for GenericStagedBuffer<T, USAGE, MULTI>
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
        self.current().buffer
    }

    fn elem_capacity(&self) -> usize {
        self.buffer.buffer.capacity.min(self.stage.elem_capacity())
    }
}

impl<T, const USAGE: u32, const MULTI: bool> Drop for GenericStagedBuffer<T, USAGE, MULTI>
where
    T: PartialEq,
{
    fn drop(&mut self) {
        unsafe {
            self.current()
                .device
                .free_memory(self.current().memory, None);
            self.current()
                .device
                .destroy_buffer(self.current().buffer, None);
        }
    }
}

// generic non-staged buffer

pub struct GenericBuffer<T, const USAGE: u32 /* vk::BufferUsageFlags */, const MULTI: bool>
where
    T: PartialEq,
{
    stage: StageBuffer<T>,
}

impl<T, const USAGE: u32, const MULTI: bool> GenericBuffer<T, USAGE, MULTI>
where
    T: PartialEq,
{
    pub fn new_with_device(device: Dev, count: usize) -> Result<Self, BufferError> {
        Ok(Self {
            stage: StageBuffer::new_with_usage(
                device.clone(),
                vk::BufferUsageFlags::from_raw(USAGE),
                count,
            )?,
        })
    }
}

// single data

impl<T, const USAGE: u32> GenericBuffer<T, USAGE, false>
where
    T: PartialEq,
{
    pub fn new_single<D>(device: &D) -> Result<Self, BufferError>
    where
        D: DerefDev,
    {
        Self::new_with_device(device.deref_dev().clone(), 1)
    }

    pub fn new_with<D>(device: &D, data: &T) -> Result<Self, BufferError>
    where
        D: DerefDev,
    {
        let mut buffer = Self::new_single(device)?;
        buffer.write(data)?;
        Ok(buffer)
    }
}

impl<T, const USAGE: u32> WriteBuffer<T> for GenericBuffer<T, USAGE, false>
where
    T: PartialEq,
{
    fn write(&mut self, data: &T) -> Result<WriteType, BufferError> {
        self.stage.write_single(0, data)
    }
}

// multi data

impl<T, const USAGE: u32> GenericBuffer<T, USAGE, true>
where
    T: PartialEq,
{
    pub fn new<D>(device: &D, count: usize) -> Result<Self, BufferError>
    where
        D: DerefDev,
    {
        Self::new_with_device(device.deref_dev().clone(), count)
    }

    pub fn new_with<D>(device: &D, data: &[T]) -> Result<Self, BufferError>
    where
        D: DerefDev,
    {
        let mut buffer = Self::new(device, data.len())?;
        buffer.write(0, data)?;
        Ok(buffer)
    }
}

impl<T, const USAGE: u32> MultiWriteBuffer<T> for GenericBuffer<T, USAGE, true>
where
    T: PartialEq,
{
    fn write(&mut self, offset: usize, data: &[T]) -> Result<WriteType, BufferError> {
        self.stage.write_slice(offset, data)
    }
}

// common impl

impl<T, const USAGE: u32, const MULTI: bool> Buffer<T> for GenericBuffer<T, USAGE, MULTI>
where
    T: PartialEq,
{
    unsafe fn update(&mut self, uri: &UpdateRecordInfo) -> bool {
        self.stage.update(uri)
    }

    fn buffer(&self) -> vk::Buffer {
        self.stage.buffer()
    }

    fn elem_capacity(&self) -> usize {
        self.stage.elem_capacity()
    }
}

// generic buffer helpers

fn create_buffer(
    device: &Dev,
    byte_size: usize,
    usage: vk::BufferUsageFlags,
    sharing_mode: vk::SharingMode,
    properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory), BufferError> {
    let mem_type = |requirements: &vk::MemoryRequirements| {
        find_mem_type(&device.memory_types, requirements, properties)
            .ok_or(BufferError::NoMemoryType(properties))
    };

    create_buffer_with_mem_type(device, byte_size, usage, sharing_mode, mem_type)
}

fn create_buffer_with_fallback(
    device: &Dev,
    byte_size: usize,
    usage: vk::BufferUsageFlags,
    sharing_mode: vk::SharingMode,
    properties: vk::MemoryPropertyFlags,
    fallback_properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory, bool), BufferError> {
    let mut non_coherent = false;
    let mem_type = |requirements: &vk::MemoryRequirements| {
        let (mem_type, _non_coherent) = upload_type(
            &device.memory_types,
            requirements,
            properties,
            fallback_properties,
        );
        non_coherent = _non_coherent;
        Ok(mem_type)
    };

    create_buffer_with_mem_type(device, byte_size, usage, sharing_mode, mem_type)
        .map(|(b, m)| (b, m, non_coherent))
}

fn create_buffer_with_mem_type<F: FnMut(&vk::MemoryRequirements) -> Result<u32, BufferError>>(
    device: &Dev,
    byte_size: usize,
    usage: vk::BufferUsageFlags,
    sharing_mode: vk::SharingMode,
    mut mem_type: F,
) -> Result<(vk::Buffer, vk::DeviceMemory), BufferError> {
    if byte_size == 0 {
        Err(BufferError::InvalidSize)
    } else {
        let buffer_info = vk::BufferCreateInfo::builder()
            .size(byte_size as u64)
            .usage(usage)
            .sharing_mode(sharing_mode);

        // Unsafe: device cannot be invalid here, unless it was deliberately invalidated or constructed illegally before
        let buffer = unsafe { device.create_buffer(&buffer_info, None) }
            .or(Err(BufferError::OutOfMemory))?;

        // Unsafe: same here
        let req = unsafe { device.get_buffer_memory_requirements(buffer) };

        let mem_type = mem_type(&req)?;

        let alloc_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(req.size)
            .memory_type_index(mem_type);

        // Unsafe: and here
        let memory = unsafe { device.allocate_memory(&alloc_info, None) }
            .or(Err(BufferError::OutOfMemory))?;

        // Unsafe: aaand here
        unsafe { device.bind_buffer_memory(buffer, memory, 0) }
            .or(Err(BufferError::OutOfMemory))?;

        Ok((buffer, memory))
    }
}

fn find_mem_type(
    available_memory_types: &[vk::MemoryType],
    requirements: &vk::MemoryRequirements,
    properties: vk::MemoryPropertyFlags,
) -> Option<u32> {
    available_memory_types
        .iter()
        .enumerate()
        .position(|(id, mem_type)| {
            requirements.memory_type_bits & (1 << id) != 0
                && mem_type.property_flags.contains(properties)
        })
        .map(|i| i as u32)
}

fn upload_type(
    available_memory_types: &[vk::MemoryType],
    requirements: &vk::MemoryRequirements,
    properties: vk::MemoryPropertyFlags,
    fallback_properties: vk::MemoryPropertyFlags,
) -> (u32, bool) {
    let primary = find_mem_type(available_memory_types, requirements, properties);
    if let Some(primary) = primary {
        (primary, false)
    } else {
        log::warn!("Primary memory properties not available, using fallback memory properties");
        let fallback = find_mem_type(available_memory_types, requirements, fallback_properties)
            .expect("Fallback memory properties not available");

        (fallback, true)
    }
}
