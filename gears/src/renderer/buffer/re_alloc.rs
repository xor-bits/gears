use crate::SimpleBuffer;
use std::mem;

pub struct ReAllocatableBuffer<T> {
    pub buffer: SimpleBuffer<T>,
    pub re_allocates: Box<[Option<(bool, SimpleBuffer<T>)>]>,
    pub re_alloc_i: usize,
}

impl<T> ReAllocatableBuffer<T> {
    pub fn new(buffer: SimpleBuffer<T>) -> Self {
        let set_count = buffer.device.set_count;
        Self {
            buffer,
            re_allocates: (0..set_count).map(|_| None).collect(),
            re_alloc_i: 0,
            /* updates: Vec::new(), */
        }
    }

    /// the old buffer wont be copied to the new one
    pub fn re_alloc(&mut self, mut buffer: SimpleBuffer<T>) {
        mem::swap(&mut self.buffer, &mut buffer);

        self.re_alloc_i = (self.re_alloc_i + 1) % self.re_allocates.len();
        self.re_allocates[self.re_alloc_i] = Some((true, buffer));
    }
}

impl<T> Drop for ReAllocatableBuffer<T> {
    fn drop(&mut self) {
        log::debug!(
            "Dropping ReAllocatableBuffer: {:?} {:?}",
            self.buffer.memory,
            self.re_allocates
                .iter()
                .filter_map(|b| b.as_ref())
                .map(|(_, b)| b.memory)
                .collect::<Vec<_>>()
        );
    }
}
