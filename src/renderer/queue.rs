use std::{marker::PhantomPinned, mem::swap, pin::Pin, ptr::NonNull};

use gfx_hal::{
    adapter::Adapter,
    prelude::QueueFamily,
    queue::{QueueFamilyId, QueueGroup},
    window::Surface,
    Backend,
};

use super::RendererError;

const PRIORITY: [f32; 1] = [1.0];
pub struct QueueFamilies {
    pub present: Option<QueueFamilyId>,
    pub graphics: Option<QueueFamilyId>,
}

pub struct Queues<B: Backend> {
    present_pin: QueueGroup<B>,
    graphics_pin: Option<QueueGroup<B>>,

    pub present: NonNull<QueueGroup<B>>,
    pub graphics: NonNull<QueueGroup<B>>,
    _pin: PhantomPinned,
}

impl QueueFamilies {
    pub fn new<B: Backend>(surface: &B::Surface, adapter: &Adapter<B>) -> Self {
        let mut queue_families = Self {
            present: None,
            graphics: None,
        };

        for queue_family in adapter.queue_families.iter() {
            if surface.supports_queue_family(queue_family) {
                queue_families.present = Some(queue_family.id());
            }
            if queue_family.queue_type().supports_graphics() {
                queue_families.graphics = Some(queue_family.id());
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

    pub fn same(&self) -> Result<bool, RendererError> {
        Ok(self
            .present
            .ok_or(RendererError::QueueFamiliesNotFinished)?
            == self
                .graphics
                .ok_or(RendererError::QueueFamiliesNotFinished)?)
    }

    pub fn get_vec<'a, B: Backend>(
        &self,
        adapter: &'a Adapter<B>,
    ) -> Result<Vec<(&'a B::QueueFamily, &[f32])>, RendererError> {
        if self.same()? {
            let present = self.present.unwrap();
            let present = adapter
                .queue_families
                .iter()
                .find(|i| i.id() == present)
                .ok_or(RendererError::AdapterMismatch)?;
            Ok(vec![(present, &PRIORITY)])
        } else {
            let present = self.present.unwrap();
            let present = adapter
                .queue_families
                .iter()
                .find(|i| i.id() == present)
                .ok_or(RendererError::AdapterMismatch)?;
            let graphics = self.present.unwrap();
            let graphics = adapter
                .queue_families
                .iter()
                .find(|i| i.id() == graphics)
                .ok_or(RendererError::AdapterMismatch)?;
            Ok(vec![(present, &PRIORITY), (graphics, &PRIORITY)])
        }
    }

    pub fn get_queues<B: Backend>(
        &self,
        mut queue_groups: Vec<QueueGroup<B>>,
    ) -> Result<Pin<Box<Queues<B>>>, RendererError> {
        if self.same()? {
            let present_pin = queue_groups
                .pop()
                .ok_or(RendererError::QueueGroupMismatch)?;

            let mut res = Box::pin(Queues::<B> {
                present_pin,
                graphics_pin: None,
                present: NonNull::dangling(),
                graphics: NonNull::dangling(),
                _pin: PhantomPinned,
            });

            let mut_ref = Pin::as_mut(&mut res);

            unsafe {
                let pin = Pin::get_unchecked_mut(mut_ref);
                pin.present = NonNull::from(&pin.present_pin);
                pin.graphics = NonNull::from(&pin.present_pin);
            }

            Ok(res)
        } else {
            let mut present_pin = queue_groups
                .pop()
                .ok_or(RendererError::QueueGroupMismatch)?;

            let mut graphics_pin = queue_groups
                .pop()
                .ok_or(RendererError::QueueGroupMismatch)?;

            if present_pin.family != self.present.unwrap() {
                swap(&mut present_pin, &mut graphics_pin);
            }

            let mut res = Box::pin(Queues::<B> {
                present_pin,
                graphics_pin: Some(graphics_pin),
                present: NonNull::dangling(),
                graphics: NonNull::dangling(),
                _pin: PhantomPinned,
            });

            let mut_ref = Pin::as_mut(&mut res);

            let pin = unsafe { Pin::get_unchecked_mut(mut_ref) };
            pin.present = NonNull::from(&pin.present_pin);
            pin.graphics = NonNull::from(pin.graphics_pin.as_ref().unwrap());

            Ok(res)
        }
    }
}
