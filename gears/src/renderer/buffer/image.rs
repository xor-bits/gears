use std::{mem::ManuallyDrop, ptr, sync::Arc};

use gfx_hal::{
    adapter::MemoryType,
    device::Device,
    format::{Aspects, Format, Swizzle},
    image::{
        FramebufferAttachment, Kind, SubresourceRange, Tiling, Usage, ViewCapabilities, ViewKind,
    },
    memory::Properties,
    Backend,
};

use super::upload_type;

pub struct Image<B: Backend> {
    device: Arc<B::Device>,

    image: ManuallyDrop<B::Image>,
    image_view: ManuallyDrop<B::ImageView>,
    memory: ManuallyDrop<B::Memory>,

    format: Format,
    usage: Usage,
}

impl<B: Backend> Image<B> {
    pub fn new_with_device(
        device: Arc<B::Device>,
        available_memory_types: &Vec<MemoryType>,
        format: Format,
        usage: Usage,
        aspects: Aspects,
        width: u32,
        height: u32,
    ) -> Self {
        let mut image = ManuallyDrop::new(
            unsafe {
                device.create_image(
                    Kind::D2(width, height, 1, 1),
                    1,
                    format,
                    Tiling::Optimal,
                    usage,
                    ViewCapabilities::empty(),
                )
            }
            .unwrap(),
        );
        let req = unsafe { device.get_image_requirements(&image) };

        let memory = ManuallyDrop::new(
            unsafe {
                device.allocate_memory(
                    upload_type(
                        available_memory_types,
                        &req,
                        Properties::DEVICE_LOCAL,
                        Properties::DEVICE_LOCAL,
                    ),
                    req.size,
                )
            }
            .unwrap(),
        );
        unsafe { device.bind_image_memory(&memory, 0, &mut image) }.unwrap();

        let image_view = ManuallyDrop::new(
            unsafe {
                device.create_image_view(
                    &image,
                    ViewKind::D2,
                    format,
                    Swizzle::NO,
                    SubresourceRange {
                        aspects,

                        level_start: 0,
                        level_count: Some(1),

                        layer_start: 0,
                        layer_count: Some(1),
                    },
                )
            }
            .unwrap(),
        );

        Self {
            device,
            image,
            image_view,
            memory,
            format,
            usage,
        }
    }

    pub fn new_depth_texture_with_device(
        device: Arc<B::Device>,
        available_memory_types: &Vec<MemoryType>,
        width: u32,
        height: u32,
    ) -> Self {
        Self::new_with_device(
            device,
            available_memory_types,
            Format::D32Sfloat,
            Usage::DEPTH_STENCIL_ATTACHMENT,
            Aspects::DEPTH,
            width,
            height,
        )
    }

    pub fn new_texture() -> Self {
        todo!()

        // Format::Rgba8Srgb
        // Usage::TRANSFER_DST | Usage::SAMPLED
        // Aspects::COLOR
    }

    pub fn view(&self) -> &B::ImageView {
        &self.image_view
    }

    pub fn framebuffer_attachment(&self) -> FramebufferAttachment {
        FramebufferAttachment {
            format: self.format,
            usage: self.usage,
            view_caps: ViewCapabilities::empty(),
        }
    }
}

impl<B: Backend> Drop for Image<B> {
    fn drop(&mut self) {
        unsafe {
            let image_view = ManuallyDrop::into_inner(ptr::read(&self.image_view));
            self.device.destroy_image_view(image_view);

            let memory = ManuallyDrop::into_inner(ptr::read(&self.memory));
            self.device.free_memory(memory);

            let image = ManuallyDrop::into_inner(ptr::read(&self.image));
            self.device.destroy_image(image);
        }
    }
}

/* fn find_format<B: Backend>(
    physical_device: &B::PhysicalDevice,
    accepted_formats: &[Format],
    tiling: Tiling,
    usage: Usage,
    view_caps: ViewCapabilities,
) {
    for accepted_format in accepted_formats {
        let properties = match physical_device.image_format_properties(
            accepted_format.clone(),
            2,
            tiling,
            usage,
            view_caps,
        ) {
            Some(f) => f,
            None => continue,
        };

        if properties.
    }
} */
