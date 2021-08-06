use std::sync::atomic::Ordering;

use crate::{
    renderer::device::Dev, Buffer, IndexBuffer, IndirectBuffer, RenderRecordInfo, UInt,
    VertexBuffer,
};
use ash::version::DeviceV1_0;

//

#[must_use]
pub struct DrawCommand;

impl DrawCommand {
    pub fn new<'a, V: PartialEq>(
        device: &'a Dev,
        rri: &'a RenderRecordInfo,
    ) -> DGDrawCommand<'a, V> {
        GDrawCommand {
            device,
            rri,

            count: 0,
            offset: 0,

            vertex_buffer: None,
            index_buffer: None,
            indirect_buffer: None,
        }
    }
}

pub type DGDrawCommand<'a, V> = GDrawCommand<'a, V, u32, false, false, 0>;

#[must_use]
pub struct GDrawCommand<'a, V, I, const VERTEX: bool, const INDEX: bool, const INDIRECT: u8>
where
    V: PartialEq,
    I: UInt,
{
    device: &'a Dev,
    rri: &'a RenderRecordInfo,

    count: u32,
    offset: u32,

    vertex_buffer: Option<&'a VertexBuffer<V>>,
    index_buffer: Option<&'a IndexBuffer<I>>,
    indirect_buffer: Option<&'a IndirectBuffer>,
}

// init

impl<'a, V, I, const INDEX: bool, const INDIRECT: u8> GDrawCommand<'a, V, I, false, INDEX, INDIRECT>
where
    V: PartialEq,
    I: UInt,
{
    pub fn vertex(
        self,
        buffer: &'a VertexBuffer<V>,
    ) -> GDrawCommand<'a, V, I, true, INDEX, INDIRECT> {
        GDrawCommand {
            device: self.device,
            rri: self.rri,

            count: self.count,
            offset: self.offset,

            vertex_buffer: Some(buffer),
            index_buffer: self.index_buffer,
            indirect_buffer: self.indirect_buffer,
        }
    }
}

impl<'a, V, I, const VERTEX: bool, const INDIRECT: u8>
    GDrawCommand<'a, V, I, VERTEX, false, INDIRECT>
where
    V: PartialEq,
    I: UInt,
{
    pub fn index<In>(
        self,
        buffer: &'a IndexBuffer<In>,
    ) -> GDrawCommand<'a, V, In, VERTEX, true, INDIRECT>
    where
        In: UInt,
    {
        GDrawCommand {
            device: self.device,
            rri: self.rri,

            count: self.count,
            offset: self.offset,

            vertex_buffer: self.vertex_buffer,
            index_buffer: Some(buffer),
            indirect_buffer: self.indirect_buffer,
        }
    }
}

impl<'a, V, I, const VERTEX: bool, const INDEX: bool> GDrawCommand<'a, V, I, VERTEX, INDEX, 0>
where
    V: PartialEq,
    I: UInt,
{
    pub fn direct(self, count: u32, offset: u32) -> GDrawCommand<'a, V, I, VERTEX, INDEX, 1> {
        GDrawCommand {
            device: self.device,
            rri: self.rri,

            count,
            offset,

            vertex_buffer: self.vertex_buffer,
            index_buffer: self.index_buffer,
            indirect_buffer: self.indirect_buffer,
        }
    }

    pub fn indirect(self, buffer: &'a IndirectBuffer) -> GDrawCommand<'a, V, I, VERTEX, INDEX, 2> {
        GDrawCommand {
            device: self.device,
            rri: self.rri,

            count: self.count,
            offset: self.offset,

            vertex_buffer: self.vertex_buffer,
            index_buffer: self.index_buffer,
            indirect_buffer: Some(buffer),
        }
    }

    /* pub fn count(self, buffer: &'a IndirectBuffer) -> GDrawCommand<'a, V, I, VERTEX, INDEX, 3> {
        GDrawCommand {
            device: self.device,
            rri: self.rri,

            count: self.count,
            offset: self.offset,

            vertex_buffer: self.vertex_buffer,
            index_buffer: self.index_buffer,
            indirect_buffer: Some(buffer),
        }
    } */
}

// bind

impl<'a, V, I, const VERTEX: bool, const INDEX: bool, const INDIRECT: u8>
    GDrawCommand<'a, V, I, VERTEX, INDEX, INDIRECT>
where
    V: PartialEq,
    I: UInt,
{
    unsafe fn bind_all(&self) {
        if VERTEX {
            self.vertex_buffer.unwrap().bind(self.rri);
        }
        if INDEX {
            self.index_buffer.unwrap().bind(self.rri);
        }
    }

    fn debug(&self) {
        self.rri.triangles.fetch_add(
            self.indirect_buffer
                .map(|b| b.count())
                .unwrap_or(self.count) as usize
                / 3,
            Ordering::SeqCst,
        );

        if self.rri.debug_calls {
            log::debug!("cmd_draw");
        }
    }
}

// draw

impl<'a, V, I, const VERTEX: bool> GDrawCommand<'a, V, I, VERTEX, false, 1>
where
    V: PartialEq,
    I: UInt,
{
    pub unsafe fn execute(self) {
        self.bind_all();
        self.debug();
        self.device
            .cmd_draw(self.rri.command_buffer, self.count, 1, self.offset, 0)
    }
}

impl<'a, V, I, const VERTEX: bool> GDrawCommand<'a, V, I, VERTEX, false, 2>
where
    V: PartialEq,
    I: UInt,
{
    pub unsafe fn execute(self) {
        self.bind_all();
        self.debug();
        let indirect_buffer = self.indirect_buffer.unwrap().buffer();
        self.device.cmd_draw_indirect(
            self.rri.command_buffer,
            indirect_buffer,
            IndirectBuffer::non_indexed(),
            1,
            0,
        )
    }
}

/* impl<'a, V, I, const VERTEX: bool> GDrawCommand<'a, V, I, VERTEX, false, 3>
where
    V: PartialEq,
    I: UInt,
{
    pub unsafe fn execute(self) {
        self.bind_all();
        self.debug();
        let indirect_buffer = self.indirect_buffer.unwrap().get_buffer();
        self.device.cmd_draw_indirect_count(
            self.rri.command_buffer,
            indirect_buffer,
            CountData::non_indexed(),
            indirect_buffer,
            CountData::draw_count(),
            !0,
            0,
        )
    }
} */

impl<'a, V, I, const VERTEX: bool> GDrawCommand<'a, V, I, VERTEX, true, 1>
where
    V: PartialEq,
    I: UInt,
{
    pub unsafe fn execute(self) {
        self.bind_all();
        self.debug();
        self.device
            .cmd_draw_indexed(self.rri.command_buffer, self.count, 1, self.offset, 0, 0)
    }
}

impl<'a, V, I, const VERTEX: bool> GDrawCommand<'a, V, I, VERTEX, true, 2>
where
    V: PartialEq,
    I: UInt,
{
    pub unsafe fn execute(self) {
        self.bind_all();
        self.debug();
        let indirect_buffer = self.indirect_buffer.unwrap().buffer();
        self.device.cmd_draw_indexed_indirect(
            self.rri.command_buffer,
            indirect_buffer,
            IndirectBuffer::indexed(),
            1,
            0,
        )
    }
}

/* impl<'a, V, I, const VERTEX: bool> GDrawCommand<'a, V, I, VERTEX, true, 3>
where
    V: PartialEq,
    I: UInt,
{
    pub unsafe fn execute(self) {
        self.bind_all();
        self.debug();
        let indirect_buffer = self.indirect_buffer.unwrap().get_buffer();
        self.device.cmd_draw_indexed_indirect_count(
            self.rri.command_buffer,
            indirect_buffer,
            CountData::indexed(),
            indirect_buffer,
            CountData::draw_count(),
            !0,
            0,
        )
    }
} */
