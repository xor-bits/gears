use crate::{Buffer, BufferError, MultiWriteBuffer, UpdateRecordInfo, WriteType};
use ash::vk;

impl<T> MultiWriteBuffer<T> for Vec<T>
where
    T: Copy,
{
    fn write(&mut self, offset: usize, data: &[T]) -> Result<WriteType, BufferError> {
        for (l, r) in self.iter_mut().skip(offset).zip(data.iter()) {
            *l = *r;
        }

        Ok(WriteType::Write)
    }
}

impl<T> Buffer<T> for Vec<T> {
    unsafe fn update(&mut self, _: &UpdateRecordInfo) -> bool {
        unimplemented!()
    }

    fn buffer(&self) -> vk::Buffer {
        unimplemented!()
    }

    fn elem_capacity(&self) -> usize {
        self.capacity()
    }
}
