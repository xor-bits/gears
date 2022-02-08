use crate::{BufferError, MultiWriteBuffer, UpdateRecordInfo};
use std::{collections::HashMap, marker::PhantomData};

struct Mod<T, const ELEMENTS: usize> {
    offset: usize,
    data: [T; ELEMENTS],
}

pub struct ModMap<Buf, T, const ELEMENTS: usize>
where
    Buf: MultiWriteBuffer<T>,
{
    modified: HashMap<usize, Mod<T, ELEMENTS>>,
    _p: PhantomData<Buf>,
}

pub struct SparseBatchBuffer<Buf, T, const ELEMENTS: usize>
where
    Buf: MultiWriteBuffer<T>,
{
    buf: Buf,
    id: usize,
    last: usize,

    free: Vec<usize>,

    _p: PhantomData<(Buf, T)>,
}

pub struct SparseBatchElement<Buf, T, const ELEMENTS: usize>
where
    Buf: MultiWriteBuffer<T>,
{
    id: usize,
    offset: usize,

    _p: PhantomData<(Buf, T)>,
}

impl<Buf, T, const ELEMENTS: usize> SparseBatchBuffer<Buf, T, ELEMENTS>
where
    Buf: MultiWriteBuffer<T>,
{
    pub fn new(buf: Buf) -> Self {
        Self {
            buf,
            id: 0,
            last: 0,

            free: Vec::new(),

            _p: PhantomData {},
        }
    }

    pub fn create_one(&mut self) -> SparseBatchElement<Buf, T, ELEMENTS> {
        let id = self.id;
        self.id += 1;
        let offset = self.acquire_one();

        SparseBatchElement {
            id,
            offset,
            _p: PhantomData {},
        }
    }

    pub fn create(&mut self, n: usize) -> Vec<SparseBatchElement<Buf, T, ELEMENTS>> {
        let id = self.id;
        self.id += n;

        self.acquire(n)
            .iter()
            .enumerate()
            .map(|(i, &offset)| SparseBatchElement {
                id: id + i,
                offset,
                _p: PhantomData {},
            })
            .collect()
    }

    pub fn remove(&mut self, element: SparseBatchElement<Buf, T, ELEMENTS>) {
        if element.offset + 1 == self.last {
            self.last -= 1;
        } else {
            self.free.push(element.offset);
        }
    }

    pub fn mod_map(&self) -> ModMap<Buf, T, ELEMENTS> {
        ModMap {
            modified: HashMap::new(),
            _p: PhantomData {},
        }
    }

    pub fn write(&mut self, mut mod_map: ModMap<Buf, T, ELEMENTS>) -> Result<(), BufferError> {
        // TODO: map and unmap
        for (_, modulation) in mod_map.modified.drain() {
            self.buf
                .write(modulation.offset * ELEMENTS, &modulation.data)?;
        };
        Ok(())
    }

    pub unsafe fn update(&mut self, uri: &UpdateRecordInfo) -> bool {
        self.buf.update(uri)
    }

    pub fn clear(&mut self) {
        self.free.clear();
        self.id = 0;
        self.last = 0;
    }

    pub fn buffer(&self) -> &'_ Buf {
        &self.buf
    }

    pub fn buffer_mut(&mut self) -> &'_ mut Buf {
        &mut self.buf
    }

    pub fn len(&self) -> usize {
        self.last
    }

    pub fn draw_count(&self) -> u32 {
        (self.len() * ELEMENTS) as u32
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

impl<Buf, T, const ELEMENTS: usize> SparseBatchElement<Buf, T, ELEMENTS>
where
    Buf: MultiWriteBuffer<T>,
{
    pub fn write(&self, mod_map: &mut ModMap<Buf, T, ELEMENTS>, data: [T; ELEMENTS]) {
        mod_map.modified.insert(
            self.id,
            Mod {
                offset: self.offset,
                data,
            },
        );
    }
}
