pub mod image;
pub mod index;
pub mod stage;
pub mod uniform;
pub mod vertex;

#[cfg(feature = "short_namespaces")]
pub use image::*;
#[cfg(feature = "short_namespaces")]
pub use index::*;
#[cfg(feature = "short_namespaces")]
pub use stage::*;
#[cfg(feature = "short_namespaces")]
pub use uniform::*;
#[cfg(feature = "short_namespaces")]
pub use vertex::*;

use ash::{version::DeviceV1_0, vk};
use log::warn;
use std::sync::Arc;

use crate::{UpdateQuery, UpdateRecordInfo};

use super::device::RenderDevice;

#[derive(Debug)]
pub enum BufferError {
    InvalidSize,
    TriedToOverflow,
    OutOfMemory,
    NoMemoryType(vk::MemoryPropertyFlags),
}

pub trait Buffer {
    fn updates(&self, uq: &UpdateQuery) -> bool;
    unsafe fn update(&mut self, uri: &UpdateRecordInfo);
    fn get(&self) -> vk::Buffer;
}

fn create_buffer(
    device: &Arc<RenderDevice>,
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
    device: &Arc<RenderDevice>,
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
    device: &Arc<RenderDevice>,
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
        warn!("Primary memory properties not available, using fallback memory properties");
        let fallback = find_mem_type(available_memory_types, requirements, fallback_properties)
            .expect("Fallback memory properties not available");

        (fallback, true)
    }
}
