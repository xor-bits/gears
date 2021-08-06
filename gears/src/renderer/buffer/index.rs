use crate::{GenericStagedBuffer, RenderRecordInfo};
use ash::{version::DeviceV1_0, vk};

const USAGE: u32 = vk::BufferUsageFlags::INDEX_BUFFER.as_raw();
const MULTI: bool = true;

pub type IndexBuffer<I> = GenericStagedBuffer<I, USAGE, MULTI>;

impl<I> IndexBuffer<I>
where
    I: UInt,
{
    pub unsafe fn bind(&self, rri: &RenderRecordInfo) {
        if rri.debug_calls {
            log::debug!("cmd_bind_index_buffer");
        }

        self.buffer.buffer.device.cmd_bind_index_buffer(
            rri.command_buffer,
            self.buffer.buffer.buffer,
            0,
            I::INDEX_TYPE,
        );
    }
}

pub trait UInt: PartialEq {
    const INDEX_TYPE: vk::IndexType;
}
impl UInt for u16 {
    const INDEX_TYPE: vk::IndexType = vk::IndexType::UINT16;
}
impl UInt for u32 {
    const INDEX_TYPE: vk::IndexType = vk::IndexType::UINT32;
}
