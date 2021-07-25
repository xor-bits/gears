use ash::vk;
use glam::{DMat2, DMat3, DMat4, DVec2, DVec3, DVec4, Mat2, Mat3, Mat4, Vec2, Vec3, Vec4};
use std::mem::size_of;

pub trait FormatOf {
    const FORMAT_OF: vk::Format;
    const OFFSET_OF: u32;
    const COUNT_OF: u32;
}

impl FormatOf for i32 {
    const FORMAT_OF: vk::Format = vk::Format::R32_SINT;
    const OFFSET_OF: u32 = size_of::<i32>() as u32;
    const COUNT_OF: u32 = 1;
}

impl FormatOf for u32 {
    const FORMAT_OF: vk::Format = vk::Format::R32_UINT;
    const OFFSET_OF: u32 = size_of::<i32>() as u32;
    const COUNT_OF: u32 = 1;
}

impl FormatOf for f32 {
    const FORMAT_OF: vk::Format = vk::Format::R32_SFLOAT;
    const OFFSET_OF: u32 = size_of::<f32>() as u32;
    const COUNT_OF: u32 = 1;
}

impl FormatOf for f64 {
    const FORMAT_OF: vk::Format = vk::Format::R64_SFLOAT;
    const OFFSET_OF: u32 = size_of::<f64>() as u32;
    const COUNT_OF: u32 = 1;
}

impl FormatOf for Vec2 {
    const FORMAT_OF: vk::Format = vk::Format::R32G32_SFLOAT;
    const OFFSET_OF: u32 = size_of::<Vec2>() as u32;
    const COUNT_OF: u32 = 1;
}

impl FormatOf for Vec3 {
    const FORMAT_OF: vk::Format = vk::Format::R32G32B32_SFLOAT;
    const OFFSET_OF: u32 = size_of::<Vec3>() as u32;
    const COUNT_OF: u32 = 1;
}

impl FormatOf for Vec4 {
    const FORMAT_OF: vk::Format = vk::Format::R32G32B32A32_SFLOAT;
    const OFFSET_OF: u32 = size_of::<Vec4>() as u32;
    const COUNT_OF: u32 = 1;
}

impl FormatOf for DVec2 {
    const FORMAT_OF: vk::Format = vk::Format::R64G64_SFLOAT;
    const OFFSET_OF: u32 = size_of::<DVec2>() as u32;
    const COUNT_OF: u32 = 1;
}

impl FormatOf for DVec3 {
    const FORMAT_OF: vk::Format = vk::Format::R64G64B64_SFLOAT;
    const OFFSET_OF: u32 = size_of::<DVec3>() as u32;
    const COUNT_OF: u32 = 1;
}

impl FormatOf for DVec4 {
    const FORMAT_OF: vk::Format = vk::Format::R64G64B64A64_SFLOAT;
    const OFFSET_OF: u32 = size_of::<DVec4>() as u32;
    const COUNT_OF: u32 = 1;
}

impl FormatOf for Mat2 {
    const FORMAT_OF: vk::Format = vk::Format::R32G32_SFLOAT;
    const OFFSET_OF: u32 = size_of::<Vec2>() as u32;
    const COUNT_OF: u32 = 2;
}

impl FormatOf for Mat3 {
    const FORMAT_OF: vk::Format = vk::Format::R32G32B32_SFLOAT;
    const OFFSET_OF: u32 = size_of::<Vec3>() as u32;
    const COUNT_OF: u32 = 3;
}

impl FormatOf for Mat4 {
    const FORMAT_OF: vk::Format = vk::Format::R32G32B32A32_SFLOAT;
    const OFFSET_OF: u32 = size_of::<Vec4>() as u32;
    const COUNT_OF: u32 = 4;
}

impl FormatOf for DMat2 {
    const FORMAT_OF: vk::Format = vk::Format::R64G64_SFLOAT;
    const OFFSET_OF: u32 = size_of::<DVec2>() as u32;
    const COUNT_OF: u32 = 2;
}

impl FormatOf for DMat3 {
    const FORMAT_OF: vk::Format = vk::Format::R64G64B64_SFLOAT;
    const OFFSET_OF: u32 = size_of::<DVec3>() as u32;
    const COUNT_OF: u32 = 3;
}

impl FormatOf for DMat4 {
    const FORMAT_OF: vk::Format = vk::Format::R64G64B64A64_SFLOAT;
    const OFFSET_OF: u32 = size_of::<DVec4>() as u32;
    const COUNT_OF: u32 = 4;
}
