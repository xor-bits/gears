/* use crate::{BufferError, MultiWriteBuffer, UpdateRecordInfo};
use std::{collections::HashMap, marker::PhantomData};

struct Mod<T, const ELEMENTS: usize> {
    offset: usize,
    data: [T; ELEMENTS],
}

pub struct DenseBatchBuffer<Buf, T, const ELEMENTS: usize>
where
    Buf: MultiWriteBuffer<T>,
{
    buf: Buf,
    id: usize,

    last: usize,
    free: Vec<usize>,
    modified: HashMap<usize, Mod<T, ELEMENTS>>,

    _p: PhantomData<(Buf, T)>,
}

pub struct DenseBatchElement<Buf, T, const ELEMENTS: usize>
where
    Buf: MultiWriteBuffer<T>,
{
    id: usize,
    offset: usize,

    _p: PhantomData<(Buf, T)>,
}

impl<Buf, T, const ELEMENTS: usize> DenseBatchBuffer<Buf, T, ELEMENTS>
where
    Buf: MultiWriteBuffer<T>,
{
    pub fn new(buf: Buf) -> Self {
        Self {
            buf,
            id: 0,

            last: 0,
            free: Vec::new(),
            modified: HashMap::new(),

            _p: PhantomData {},
        }
    }

    pub fn create_one(&mut self) -> DenseBatchElement<Buf, T, ELEMENTS> {
        let id = self.id;
        self.id += 1;
        let offset = self.acquire_one();

        DenseBatchElement {
            id,
            offset,
            _p: PhantomData {},
        }
    }

    pub fn create(&mut self, n: usize) -> Vec<DenseBatchElement<Buf, T, ELEMENTS>> {
        let id = self.id;
        self.id += n;

        self.acquire(n)
            .iter()
            .enumerate()
            .map(|(i, &offset)| DenseBatchElement {
                id: id + i,
                offset,
                _p: PhantomData {},
            })
            .collect()
    }

    pub fn remove(&mut self, element: DenseBatchElement<Buf, T, ELEMENTS>) {
        self.free.push(element.offset);
    }

    pub fn write(&mut self) -> Result<(), BufferError> {
        self.compact();

        // TODO: map and unmap
        Ok(for (_, modulation) in self.modified.drain() {
            self.buf
                .write(modulation.offset * ELEMENTS, &modulation.data)?;
        })
    }

    pub unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        self.buf.update(uri)
    }

    pub fn clear(&mut self) {
        self.free.clear();
        self.modified.clear();
        self.id = 0;
        self.last = 0;
    }

    fn compact(&mut self) {}

    pub fn buffer(&self) -> &'_ Buf {
        &self.buf
    }

    pub fn buffer_mut(&mut self) -> &'_ mut Buf {
        &mut self.buf
    }

    fn acquire_one(&mut self) -> usize {
        if let Some(i) = self.free.pop() {
            i
        } else {
            let i = self.last;
            self.last += 1;
            i
        }
    }

    fn acquire(&mut self, count: usize) -> Vec<usize> {
        let from_free_slots = count.min(self.free.len());
        let new_slots = count - from_free_slots;

        let first_new_slot = self.last;
        self.last += new_slots;

        self.free
            .drain(0..from_free_slots)
            .chain(first_new_slot..first_new_slot + new_slots)
            .collect()
    }
}

impl<Buf, T, const ELEMENTS: usize> DenseBatchElement<Buf, T, ELEMENTS>
where
    Buf: MultiWriteBuffer<T>,
{
    pub fn write(&self, buffer: &mut DenseBatchBuffer<Buf, T, ELEMENTS>, data: [T; ELEMENTS]) {
        buffer.modified.insert(
            self.id,
            Mod {
                offset: self.offset,
                data,
            },
        );
    }
}
 */
