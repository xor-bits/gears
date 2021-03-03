// extern crate gfx_backend_empty as back;
extern crate gfx_backend_vulkan as back;

use colored::Colorize;
use gfx_hal::{
    adapter::Adapter,
    prelude::{PhysicalDevice, QueueFamily},
    window::Surface,
    Instance,
};
use winit::{dpi::LogicalSize, event_loop::EventLoop, window::WindowBuilder};

mod log;

struct QueueFamilies<'a, B: gfx_hal::Backend> {
    present: Option<&'a B::QueueFamily>,
    graphics: Option<&'a B::QueueFamily>,
}

impl<'a, B: gfx_hal::Backend> QueueFamilies<'a, B> {
    pub fn new(surface: &B::Surface, adapter: &'a Adapter<B>) -> Self {
        let mut queue_families = Self {
            present: None,
            graphics: None,
        };

        for queue_family in adapter.queue_families.iter() {
            if surface.supports_queue_family(queue_family) {
                queue_families.present = Some(queue_family);
            }
            if queue_family.queue_type().supports_graphics() {
                queue_families.graphics = Some(queue_family);
            }
            if queue_families.finished() {
                break;
            }
        }

        queue_families
    }

    pub fn finished(&self) -> bool {
        self.present.is_some() && self.graphics.is_some()
    }

    pub fn get(&self) {}
}

fn adapter_to_string<B: gfx_hal::Backend>(adapter: &Adapter<B>) -> String {
    format!(
        "{} (type:{})",
        adapter.info.name.cyan(),
        format!("{:?}", adapter.info.device_type).green(),
    )
}

pub fn init() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Gears")
        .with_inner_size(LogicalSize::new(900, 500))
        .build(&event_loop)
        .unwrap();

    let instance = back::Instance::create("Gears", 1).expect("Failed to create an instance!");
    let surface = unsafe { instance.create_surface(&window) }.unwrap();

    let mut adapter_names = Vec::new();
    let adapter = {
        let mut suitable_adapters = instance
            .enumerate_adapters()
            .into_iter()
            .filter_map(|adapter| {
                let queue_families = QueueFamilies::new(&surface, &adapter);
                adapter_names.push(adapter.info.name.clone());
                if !queue_families.finished() {
                    None
                } else {
                    let device_type = adapter.info.device_type.clone();
                    Some((
                        adapter,
                        match device_type {
                            gfx_hal::adapter::DeviceType::DiscreteGpu => 4,
                            gfx_hal::adapter::DeviceType::IntegratedGpu => 3,
                            gfx_hal::adapter::DeviceType::VirtualGpu => 2,
                            gfx_hal::adapter::DeviceType::Cpu => 1,
                            gfx_hal::adapter::DeviceType::Other => 0,
                        },
                    ))
                }
            })
            .collect::<Vec<_>>();

        suitable_adapters.sort_by(|lhs, rhs| lhs.1.cmp(&rhs.1));
        let last = suitable_adapters.remove(suitable_adapters.len() - 1);
        last.0
    };
    log_info!("Selected {}", adapter_to_string(&adapter));
    let queue_families = QueueFamilies::new(&surface, &adapter);

    let gpu = unsafe {
        adapter.physical_device.open(
            &[(queue_families.graphics.unwrap(), &[1.0])],
            gfx_hal::Features::empty(),
        )
    }
    .unwrap();

    unsafe { instance.destroy_surface(surface) };
}
