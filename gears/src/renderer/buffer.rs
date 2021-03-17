pub mod index;
pub mod uniform;
pub mod vertex;

pub use index::*;
pub use uniform::*;
pub use vertex::*;

use gfx_hal::{
    adapter::MemoryType,
    memory::{Properties, Requirements},
    Backend, MemoryTypeId,
};
use log::warn;

pub trait Buffer<B: Backend> {
    /* fn new<T>(
        device: &B::Device,
        buffer_manager: &mut BufferManager,
        available_memory_types: &Vec<MemoryType>,
        size: usize,
    ) -> Self; */
    fn destroy(self, device: &B::Device);
}

fn find_mem_type(
    available_memory_types: &Vec<MemoryType>,
    requirements: &Requirements,
    properties: Properties,
) -> Option<MemoryTypeId> {
    available_memory_types
        .iter()
        .enumerate()
        .position(|(id, mem_type)| {
            // type_mask is a bit field where each bit represents a memory type. If the bit is set
            // to 1 it means we can use that type for our buffer. So this code finds the first
            // memory type that has a `1` (or, is allowed), and is visible to the CPU.
            requirements.type_mask & (1 << id) != 0 && mem_type.properties.contains(properties)
        })
        .map(|id| id.into())
}

fn upload_type(
    available_memory_types: &Vec<MemoryType>,
    requirements: &Requirements,
    properties: Properties,
    fallback_properties: Properties,
) -> MemoryTypeId {
    find_mem_type(available_memory_types, requirements, properties).unwrap_or_else(|| {
        warn!("Primary memory properties not available, using fallback memory properties");
        find_mem_type(available_memory_types, requirements, fallback_properties)
            .expect("Fallback memory properties not available")
    })
}
