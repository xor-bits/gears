use self::{any::GPUPicker, unsuitable::UnsuitableGPU};

use super::{ContextError, ContextGPUPick};
use crate::renderer::queue::QueueFamilies;
use any::AnyGPU;
use bytesize::ByteSize;
use colored::Colorize;
use score::GPUScore;
use std::{fmt::Write, sync::Arc};
use suitable::SuitableGPU;
use vulkano::{device::physical::PhysicalDevice, instance::Instance, swapchain::Surface};
use winit::window::Window;

pub mod any;
pub mod score;
pub mod suitable;
pub mod unsuitable;

// pick

impl SuitableGPU {
    pub fn pick(
        instance: &Arc<Instance>,
        surface: &Arc<Surface<Arc<Window>>>,
        pick: ContextGPUPick,
    ) -> Result<Self, ContextError> {
        let p_devices = PhysicalDevice::enumerate(instance)
            .map(|p_device| {
                let queue_families = QueueFamilies::new(surface, p_device)?;
                let score = GPUScore::new(p_device);

                if queue_families.is_some() {
                    Ok(GPUPicker::Suitable(SuitableGPU {
                        instance: instance.clone(),
                        p_device: p_device.index(),
                        score,
                    }))
                } else {
                    Ok(GPUPicker::Unsuitable(UnsuitableGPU {
                        instance: instance.clone(),
                        p_device: p_device.index(),
                        score,
                    }))
                }
            })
            .collect::<Result<Vec<GPUPicker>, ContextError>>()?;

        Self::pick_best(&p_devices, pick)
    }

    fn list<'a>(
        p_devices: impl Iterator<Item = &'a dyn AnyGPU>,
        ignore_invalid: bool,
    ) -> String {
        let mut buf = String::new();
        for (i, p_device) in p_devices
            .filter(|p_device| {
                if !ignore_invalid {
                    true
                } else {
                    p_device.suitable()
                }
            })
            .enumerate()
        {
            let suitable = if p_device.suitable() {
                "[\u{221a}]".green()
            } else {
                "[X]".red()
            };
            let device = p_device.device().properties();
            let name = device.device_name.blue();
            let ty = format!("{:?}", device.device_type).blue();
            let mem = ByteSize::b(p_device.score().memory).to_string().blue();
            let score = p_device.score().score().to_string().yellow();

            writeln!(buf, "- GPU index: {}", i).unwrap();
            writeln!(buf, "  - suitable: {}", suitable).unwrap();
            writeln!(buf, "  - name: {}", name).unwrap();
            writeln!(buf, "  - type: {}", ty).unwrap();
            writeln!(buf, "  - memory: {}", mem).unwrap();
            writeln!(buf, "  - automatic score: {}", score).unwrap();
        }
        buf
    }

    fn pick_best(
        p_devices: &Vec<GPUPicker>,
        pick: ContextGPUPick,
    ) -> Result<SuitableGPU, ContextError> {
        let mut suitable = p_devices
            .iter()
            .filter_map(|p_device| match p_device {
                GPUPicker::Suitable(d) => Some(d.clone()),
                GPUPicker::Unsuitable(_) => None,
            })
            .collect::<Vec<SuitableGPU>>();
        let all_iter = p_devices.iter().map(|d| d as &dyn AnyGPU);
        let suitable_iter = suitable.iter().map(|d| d as &dyn AnyGPU);

        let p_device = if suitable.len() == 0 {
            None
        } else if suitable.len() == 1 {
            if pick == ContextGPUPick::Manual {
                log::warn!(
                    "ContextGPUPick was set to Manual but only one suitable GPU was available"
                )
            }

            Some(suitable.remove(0))
        } else if pick == ContextGPUPick::Manual {
            println!("Pick a GPU:\n{}", Self::list(suitable_iter, true,));

            let stdin = std::io::stdin();
            let mut stdout = std::io::stdout();
            let i = loop {
                print!("Number: ");
                std::io::Write::flush(&mut stdout).unwrap();
                let mut buf = String::new();
                stdin.read_line(&mut buf).unwrap();

                match buf.trim_end().parse::<usize>() {
                    Ok(i) => {
                        if i >= suitable.len() {
                            println!(
                                "{} is not a valid GPU index between 0 and {}",
                                i,
                                suitable.len() + 1
                            );
                        } else {
                            break i;
                        }
                    }
                    Err(_) => {
                        println!("'{}' is not a valid GPU index", buf);
                    }
                }
            };

            // optionally save the manual pick
            // p_device.properties().device_uuid;

            Some(suitable.remove(i))
        } else {
            suitable.sort();
            suitable.pop()
        };

        match p_device {
            Some(p_device) => {
                log::info!(
                    "Picked: GPU index: {} ({}) from:\n{}",
                    p_device.device().index(),
                    p_device.device().properties().device_name.blue(),
                    Self::list(all_iter, false)
                );
                Ok(p_device)
            }
            None => {
                log::error!(
                    "None of the GPUs (bellow) are suitable:\n{}",
                    Self::list(all_iter, false)
                );
                Err(ContextError::NoSuitableGPUs)
            }
        }
    }
}
