use crate::GenericBuffer;
use ash::vk;

const USAGE: u32 = vk::BufferUsageFlags::UNIFORM_BUFFER.as_raw();
const MULTI: bool = false;

pub type UniformBuffer<T> = GenericBuffer<T, USAGE, MULTI>;
