pub mod uniform;
pub mod vertex;

pub use uniform::UniformBuffer;
pub use vertex::VertexBuffer;

use gfx_hal::{
    adapter::MemoryType,
    device::{BindError, Device},
    memory::{Properties, Requirements},
    Backend, MemoryTypeId,
};
use log::{debug, warn};

type BufferID = usize;

pub struct BufferManager {
    next_id: BufferID,
    bound_id: BufferID,
}

pub trait Buffer<B: Backend> {
    /* fn new<T>(
        device: &B::Device,
        buffer_manager: &mut BufferManager,
        available_memory_types: &Vec<MemoryType>,
        size: usize,
    ) -> Self; */
    fn destroy(self, device: &B::Device);
}

impl BufferManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            bound_id: 0,
        }
    }

    fn next(&mut self) -> BufferID {
        let result = self.next_id;
        self.next_id += 1;
        result
    }

    fn bind_memory<B: Backend>(
        &mut self,
        device: &B::Device,
        memory: &B::Memory,
        offset: u64,
        buffer: &mut B::Buffer,
        id: BufferID,
    ) -> Result<(), BindError> {
        if self.bound_id != id {
            self.bound_id = id;
            debug!("Binding memory: {}", id);
            unsafe { device.bind_buffer_memory(memory, offset, buffer) }
        } else {
            Ok(())
        }
    }
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
