pub mod image;
pub mod index;
pub mod uniform;
pub mod vertex;

#[cfg(feature = "short_namespaces")]
pub use image::*;
#[cfg(feature = "short_namespaces")]
pub use index::*;
#[cfg(feature = "short_namespaces")]
pub use uniform::*;
#[cfg(feature = "short_namespaces")]
pub use vertex::*;

use ash::vk;
use log::warn;

#[derive(Debug)]
pub enum BufferError {
    InvalidSize,
    TriedToOverflow,
    OutOfMemory,
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
) -> u32 {
    find_mem_type(available_memory_types, requirements, properties).unwrap_or_else(|| {
        warn!("Primary memory properties not available, using fallback memory properties");
        find_mem_type(available_memory_types, requirements, fallback_properties)
            .expect("Fallback memory properties not available")
    })
}
