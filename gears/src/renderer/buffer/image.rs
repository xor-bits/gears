use ash::{version::DeviceV1_0, vk};
use bitflags::bitflags;
use log::*;
use std::{marker::PhantomData, sync::Arc};

use crate::Renderer;

use super::{upload_type, BufferError};

pub trait BaseFormat {
    fn format(&self) -> vk::Format;
}

bitflags! {
    pub struct ImageUsage: u8 {
        const READ = 1;
        const WRITE = 2;
        const BOTH = 3;
    }
}

pub enum ImageFormat<T> {
    R,
    RG,
    RGB,
    RGBA,
    D,

    _P(PhantomData<T>),
}

pub struct ImageBuilder<'a> {
    device: Arc<ash::Device>,
    available_memory_types: &'a [vk::MemoryType],
}

pub struct ImageBuilder1D<'a> {
    base: ImageBuilder<'a>,
    width: u32,
}

pub struct ImageBuilder2D<'a> {
    base: ImageBuilder<'a>,
    width: u32,
    height: u32,
}

pub struct ImageBuilder3D<'a> {
    base: ImageBuilder<'a>,
    width: u32,
    height: u32,
    depth: u32,
}

pub struct Image {
    device: Arc<ash::Device>,

    image: vk::Image,
    image_view: vk::ImageView,
    memory: Option<vk::DeviceMemory>,

    owns_image: bool,
}

impl<T> Into<vk::Format> for ImageFormat<T>
where
    ImageFormat<T>: BaseFormat,
{
    fn into(self) -> vk::Format {
        self.format()
    }
}

impl BaseFormat for ImageFormat<f32> {
    fn format(&self) -> vk::Format {
        match self {
            ImageFormat::R => vk::Format::R8_SRGB,
            ImageFormat::RG => vk::Format::R8G8_SRGB,
            ImageFormat::RGB => vk::Format::R8G8B8_SRGB,
            ImageFormat::RGBA => vk::Format::R8G8B8A8_SRGB,
            ImageFormat::D => vk::Format::D32_SFLOAT,

            ImageFormat::_P(_) => unreachable!(),
        }
    }
}

impl BaseFormat for ImageFormat<u8> {
    fn format(&self) -> vk::Format {
        match self {
            ImageFormat::R => vk::Format::R8_UINT,
            ImageFormat::RG => vk::Format::R8G8_UINT,
            ImageFormat::RGB => vk::Format::R8G8B8_UINT,
            ImageFormat::RGBA => vk::Format::R8G8B8A8_UINT,
            ImageFormat::D => vk::Format::D24_UNORM_S8_UINT,

            ImageFormat::_P(_) => unreachable!(),
        }
    }
}

impl BaseFormat for ImageFormat<i8> {
    fn format(&self) -> vk::Format {
        match self {
            ImageFormat::R => vk::Format::R8_SINT,
            ImageFormat::RG => vk::Format::R8G8_SINT,
            ImageFormat::RGB => vk::Format::R8G8B8_SINT,
            ImageFormat::RGBA => vk::Format::R8G8B8A8_SINT,
            ImageFormat::D => vk::Format::D24_UNORM_S8_UINT,

            ImageFormat::_P(_) => unreachable!(),
        }
    }
}

impl BaseFormat for ImageFormat<u16> {
    fn format(&self) -> vk::Format {
        match self {
            ImageFormat::R => vk::Format::R16_UINT,
            ImageFormat::RG => vk::Format::R16G16_UINT,
            ImageFormat::RGB => vk::Format::R16G16B16_UINT,
            ImageFormat::RGBA => vk::Format::R16G16B16A16_UINT,
            ImageFormat::D => vk::Format::D24_UNORM_S8_UINT,

            ImageFormat::_P(_) => unreachable!(),
        }
    }
}

impl BaseFormat for ImageFormat<i16> {
    fn format(&self) -> vk::Format {
        match self {
            ImageFormat::R => vk::Format::R16_SINT,
            ImageFormat::RG => vk::Format::R16G16_SINT,
            ImageFormat::RGB => vk::Format::R16G16B16_SINT,
            ImageFormat::RGBA => vk::Format::R16G16B16A16_SINT,
            ImageFormat::D => vk::Format::D24_UNORM_S8_UINT,

            ImageFormat::_P(_) => unreachable!(),
        }
    }
}

impl BaseFormat for ImageFormat<u32> {
    fn format(&self) -> vk::Format {
        match self {
            ImageFormat::R => vk::Format::R32_UINT,
            ImageFormat::RG => vk::Format::R32G32_UINT,
            ImageFormat::RGB => vk::Format::R32G32B32_UINT,
            ImageFormat::RGBA => vk::Format::R32G32B32A32_UINT,
            ImageFormat::D => vk::Format::D24_UNORM_S8_UINT,

            ImageFormat::_P(_) => unreachable!(),
        }
    }
}

impl BaseFormat for ImageFormat<i32> {
    fn format(&self) -> vk::Format {
        match self {
            ImageFormat::R => vk::Format::R32_SINT,
            ImageFormat::RG => vk::Format::R32G32_SINT,
            ImageFormat::RGB => vk::Format::R32G32B32_SINT,
            ImageFormat::RGBA => vk::Format::R32G32B32A32_SINT,
            ImageFormat::D => vk::Format::D24_UNORM_S8_UINT,

            ImageFormat::_P(_) => unreachable!(),
        }
    }
}

impl<'a> ImageBuilder<'a> {
    pub fn new(renderer: &'a Renderer) -> Self {
        Self {
            device: renderer.device.clone(),
            available_memory_types: &renderer.memory_properties.memory_types,
        }
    }

    pub fn new_with_device(
        device: Arc<ash::Device>,
        available_memory_types: &'a [vk::MemoryType],
    ) -> Self {
        Self {
            device,
            available_memory_types,
        }
    }

    pub fn with_width(self, width: u32) -> ImageBuilder1D<'a> {
        ImageBuilder1D::<'a> { base: self, width }
    }

    fn get(
        image_usage: ImageUsage,
        image_format: vk::Format,
    ) -> (vk::ImageAspectFlags, vk::ImageUsageFlags) {
        let depth = match image_format {
            vk::Format::D16_UNORM
            | vk::Format::D16_UNORM_S8_UINT
            | vk::Format::D24_UNORM_S8_UINT
            | vk::Format::D32_SFLOAT
            | vk::Format::D32_SFLOAT_S8_UINT
            | vk::Format::X8_D24_UNORM_PACK32 => true,
            _ => false,
        };
        let mut usage = vk::ImageUsageFlags::empty();
        if image_usage.contains(ImageUsage::READ) {
            usage |= vk::ImageUsageFlags::SAMPLED;
        }
        if image_usage.contains(ImageUsage::WRITE) {
            usage |= if depth {
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
            } else {
                vk::ImageUsageFlags::COLOR_ATTACHMENT
            };
        }

        let aspects = if depth {
            vk::ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };

        (aspects, usage)
    }

    pub fn build_with_image<T>(
        self,
        image: vk::Image,
        image_usage: ImageUsage,
        image_format: T,
    ) -> Result<Image, BufferError>
    where
        T: Into<vk::Format>,
    {
        let format = image_format.into();
        let (aspects, _) = ImageBuilder::get(image_usage, format);

        Image::new_with_image(
            self.device,
            &self.available_memory_types,
            image,
            format,
            aspects,
            vk::ImageType::TYPE_2D,
            false,
        )
    }
}

impl<'a> ImageBuilder1D<'a> {
    pub fn with_height(self, height: u32) -> ImageBuilder2D<'a> {
        ImageBuilder2D::<'a> {
            base: self.base,
            width: self.width,
            height,
        }
    }

    pub fn build<T>(self, image_usage: ImageUsage, image_format: T) -> Result<Image, BufferError>
    where
        T: Into<vk::Format>,
    {
        if self.width == 0 {
            Err(BufferError::InvalidSize)
        } else {
            let format = image_format.into();
            let (aspects, usage) = ImageBuilder::get(image_usage, format);
            let extent = vk::Extent3D {
                width: self.width,
                height: 1,
                depth: 1,
            };

            Image::new(
                self.base.device,
                &self.base.available_memory_types,
                format,
                usage,
                aspects,
                extent,
                vk::ImageType::TYPE_1D,
            )
        }
    }
}

impl<'a> ImageBuilder2D<'a> {
    pub fn with_depth(self, depth: u32) -> ImageBuilder3D<'a> {
        ImageBuilder3D::<'a> {
            base: self.base,
            width: self.width,
            height: self.height,
            depth,
        }
    }

    pub fn build<T>(self, image_usage: ImageUsage, image_format: T) -> Result<Image, BufferError>
    where
        T: Into<vk::Format>,
    {
        if self.width == 0 || self.height == 0 {
            Err(BufferError::InvalidSize)
        } else {
            let format = image_format.into();
            let (aspects, usage) = ImageBuilder::get(image_usage, format);
            let extent = vk::Extent3D {
                width: self.width,
                height: self.height,
                depth: 1,
            };

            Image::new(
                self.base.device,
                &self.base.available_memory_types,
                format,
                usage,
                aspects,
                extent,
                vk::ImageType::TYPE_2D,
            )
        }
    }
}

impl<'a> ImageBuilder3D<'a> {
    pub fn build<T>(self, image_usage: ImageUsage, image_format: T) -> Result<Image, BufferError>
    where
        T: Into<vk::Format>,
    {
        if self.width == 0 || self.height == 0 || self.depth == 0 {
            Err(BufferError::InvalidSize)
        } else {
            let format = image_format.into();
            let (aspects, usage) = ImageBuilder::get(image_usage, format);
            let extent = vk::Extent3D {
                width: self.width,
                height: self.height,
                depth: self.depth,
            };

            Image::new(
                self.base.device,
                &self.base.available_memory_types,
                format,
                usage,
                aspects,
                extent,
                vk::ImageType::TYPE_3D,
            )
        }
    }
}

impl Image {
    fn new(
        device: Arc<ash::Device>,
        available_memory_types: &[vk::MemoryType],
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspects: vk::ImageAspectFlags,
        extent: vk::Extent3D,
        image_type: vk::ImageType,
    ) -> Result<Self, BufferError> {
        let image_info = vk::ImageCreateInfo::builder()
            .format(format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .extent(extent)
            .image_type(image_type)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .mip_levels(1)
            .array_layers(1)
            .build();

        let image = unsafe { device.create_image(&image_info, None) }.or_else(|err| {
            error!("Image ({:?}) creation failed: {:?}", image_info, err);
            Err(BufferError::OutOfMemory)
        })?;

        Self::new_with_image(
            device,
            available_memory_types,
            image,
            format,
            aspects,
            image_type,
            true,
        )
    }

    fn new_with_image(
        device: Arc<ash::Device>,
        available_memory_types: &[vk::MemoryType],
        image: vk::Image,
        format: vk::Format,
        aspects: vk::ImageAspectFlags,
        image_type: vk::ImageType,
        owns_image: bool,
    ) -> Result<Self, BufferError> {
        let memory = if owns_image {
            let req = unsafe { device.get_image_memory_requirements(image) };

            let memory_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(req.size)
                .memory_type_index(upload_type(
                    available_memory_types,
                    &req,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                ));

            let memory = unsafe { device.allocate_memory(&memory_info, None) }
                .or(Err(BufferError::OutOfMemory))?;

            unsafe { device.bind_image_memory(image, memory, 0) }
                .or(Err(BufferError::OutOfMemory))?;

            Some(memory)
        } else {
            None
        };

        let image_view_info = vk::ImageViewCreateInfo::builder()
            .image(image)
            .format(format)
            .components(
                vk::ComponentMapping::builder()
                    .r(vk::ComponentSwizzle::R)
                    .g(vk::ComponentSwizzle::G)
                    .b(vk::ComponentSwizzle::B)
                    .a(vk::ComponentSwizzle::A)
                    .build(),
            )
            .view_type(match image_type {
                vk::ImageType::TYPE_1D => vk::ImageViewType::TYPE_1D,
                vk::ImageType::TYPE_2D => vk::ImageViewType::TYPE_2D,
                _ /* vk::ImageType::TYPE_3D */ => vk::ImageViewType::TYPE_3D,
            })
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(aspects)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            );

        let image_view = unsafe { device.create_image_view(&image_view_info, None) }
            .or(Err(BufferError::OutOfMemory))?;

        Ok(Self {
            device,

            image,
            image_view,
            memory,

            owns_image,
        })
    }

    pub fn view(&self) -> vk::ImageView {
        self.image_view
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_image_view(self.image_view, None);
            self.memory
                .map(|memory| self.device.free_memory(memory, None));

            if self.owns_image {
                self.device.destroy_image(self.image, None);
            }
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
