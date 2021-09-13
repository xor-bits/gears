use super::{device::Dev, Recorder};
use anyhow::Result;
use std::{
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use vulkano::{
    buffer::{
        cpu_access::{ReadLock, WriteLock},
        CpuAccessibleBuffer, DeviceLocalBuffer,
    },
    memory::Content,
    DeviceSize,
};

pub use vulkano::buffer::BufferUsage;

pub struct StagedBuffer<T: ?Sized> {
    pub stage: Arc<CpuAccessibleBuffer<T>>,
    pub local: Arc<DeviceLocalBuffer<T>>,
    updates: AtomicBool,
}

impl<T: ?Sized> Deref for StagedBuffer<T> {
    type Target = Arc<DeviceLocalBuffer<T>>;

    fn deref(&self) -> &Self::Target {
        &self.local
    }
}

impl<T: ?Sized> DerefMut for StagedBuffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.local
    }
}

impl<T> StagedBuffer<T>
where
    T: Default + Copy + Send + Sync + 'static,
{
    pub fn new(device: &Dev, usage: BufferUsage) -> Result<Self> {
        Self::from_data(device, usage, T::default())
    }
}

impl<T> StagedBuffer<T>
where
    T: Copy + Send + Sync + 'static,
{
    pub fn from_data(device: &Dev, usage: BufferUsage, data: T) -> Result<Self> {
        let (stage_usage, local_usage) = make_usage(usage);

        let stage =
            CpuAccessibleBuffer::from_data(device.logical().clone(), stage_usage, false, data)?;
        let local = make_local(device, local_usage)?;

        let buffer = Self {
            stage,
            local,
            updates: AtomicBool::new(true),
        };

        Ok(buffer)
    }
}

impl<T> StagedBuffer<[T]>
where
    T: Send + Sync + 'static,
{
    pub fn from_iter<I>(device: &Dev, usage: BufferUsage, data: I) -> Result<Self>
    where
        I: ExactSizeIterator<Item = T>,
    {
        let (stage_usage, local_usage) = make_usage(usage);
        let len = data.len();

        let stage =
            CpuAccessibleBuffer::from_iter(device.logical().clone(), stage_usage, false, data)?;
        let local = make_local_array(device, local_usage, len as u64)?;

        let buffer = Self {
            stage,
            local,
            updates: AtomicBool::new(true),
        };

        Ok(buffer)
    }
}

impl<T> StagedBuffer<T>
where
    T: ?Sized + Content + Send + Sync + 'static,
{
    /// copy data from the device local buffer back to the stage buffer
    ///
    /// used when the gpu writes to the device local buffer
    pub fn copy_to_stage(&self, recorder: &mut Recorder<false>) -> Result<()> {
        recorder
            .record()
            .copy_buffer(self.local.clone(), self.stage.clone())?;
        Ok(())
    }

    /// copy the stage buffer to the device local buffer
    ///
    /// `update` will call this after writing to the stage buffer
    pub fn copy_to_local(&self, recorder: &mut Recorder<false>) -> Result<()> {
        recorder
            .record()
            .copy_buffer(self.stage.clone(), self.local.clone())?;
        Ok(())
    }

    /// update sends data from the stage to the device local buffer
    ///
    /// must be called after creation
    pub fn update(&self, recorder: &mut Recorder<false>) -> Result<()> {
        // update only if there was any updates
        if self.updates.swap(false, Ordering::SeqCst) {
            // command to copy the stage buffer to the device local buffer
            self.copy_to_local(recorder)
        } else {
            // do not update if there is nothing to update
            Ok(())
        }
    }

    /// multiple writes will result in multiple copy operations
    pub fn write(&self, recorder: &mut Recorder<false>) -> Result<WriteLock<T>> {
        // store here and swap in the update will cancel out
        // (unless it gets updated between the store and swap but it wont matter)
        self.updates.store(true, Ordering::SeqCst);
        self.update(recorder)?;

        // acquire the write lock
        let lock = self.stage.write()?;
        Ok(lock)
    }
}

impl<T> StagedBuffer<T>
where
    T: ?Sized + Content + 'static,
{
    pub fn read(&self) -> Result<ReadLock<T>> {
        Ok(self.stage.read()?)
    }
}

fn make_usage(usage: BufferUsage) -> (BufferUsage, BufferUsage) {
    (
        BufferUsage {
            transfer_source: true,
            ..usage
        },
        BufferUsage {
            transfer_destination: true,
            ..usage
        },
    )
}

fn make_local<T>(device: &Dev, local_usage: BufferUsage) -> Result<Arc<DeviceLocalBuffer<T>>> {
    Ok(DeviceLocalBuffer::new(
        device.logical().clone(),
        local_usage,
        [device.queues.graphics.family()].iter().cloned(),
    )?)
}

fn make_local_array<T>(
    device: &Dev,
    local_usage: BufferUsage,
    len: DeviceSize,
) -> Result<Arc<DeviceLocalBuffer<[T]>>> {
    Ok(DeviceLocalBuffer::array(
        device.logical().clone(),
        len,
        local_usage,
        [device.queues.graphics.family()].iter().cloned(),
    )?)
}

/* pub trait ResizeBuffer<T>
where
    Self: Sized,
{
    type ResultType;

    fn resize_with_iter<I>(
        &self,
        device: &Dev,
        usage: BufferUsage,
        append: I,
    ) -> Result<Self::ResultType>
    where
        I: ExactSizeIterator<Item = T>;
}

impl<T> ResizeBuffer<T> for CpuAccessibleBuffer<[T]>
where
    T: Clone + 'static,
{
    type ResultType = Arc<Self>;

    fn resize_with_iter<I>(
        &self,
        device: &Dev,
        usage: BufferUsage,
        append: I,
    ) -> Result<Self::ResultType>
    where
        I: ExactSizeIterator<Item = T>,
    {
        let lock = self.read()?;
        let data = (*lock).iter().cloned().chain(append).collect::<Box<_>>();
        let iter = data.into_iter().cloned();

        let buffer = Self::from_iter(device.logical().clone(), usage, false, iter)?;

        Ok(buffer)
    }
}

impl<T> ResizeBuffer<T> for StagedBuffer<[T]>
where
    T: Clone + 'static,
{
    type ResultType = Self;

    fn resize_with_iter<I>(
        &self,
        device: &Dev,
        usage: BufferUsage,
        append: I,
    ) -> Result<Self::ResultType>
    where
        I: ExactSizeIterator<Item = T>,
    {
        let lock = self.read()?;
        let data = (*lock).iter().cloned().chain(append).collect::<Box<_>>();
        let iter = data.into_iter().cloned();

        let buffer = Self::from_iter(device, usage, iter)?;

        Ok(buffer)
    }
} */
