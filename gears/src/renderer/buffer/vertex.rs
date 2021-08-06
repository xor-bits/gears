use crate::{GenericStagedBuffer, RenderRecordInfo};
use ash::{version::DeviceV1_0, vk};

const USAGE: u32 = vk::BufferUsageFlags::VERTEX_BUFFER.as_raw();
const MULTI: bool = true;

pub type VertexBuffer<T> = GenericStagedBuffer<T, USAGE, MULTI>;

impl<T> VertexBuffer<T>
where
    T: PartialEq,
{
    pub unsafe fn bind(&self, rri: &RenderRecordInfo) {
        let buffer = [self.current().buffer];
        let offsets = [0];

        if rri.debug_calls {
            log::debug!("cmd_bind_vertex_buffers");
        }

        self.current()
            .device
            .cmd_bind_vertex_buffers(rri.command_buffer, 0, &buffer, &offsets);
    }
}
