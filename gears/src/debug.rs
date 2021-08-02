use ash::{extensions::ext, prelude::VkResult, vk};
use log::{log, Level};
use std::{borrow::Cow, ffi::CStr};

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *data;
    let message_id_number: i32 = callback_data.message_id_number as i32;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    let level = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => Level::Error,
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => Level::Info,
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => Level::Trace,
        _ /* vk::DebugUtilsMessageSeverityFlagsEXT::WARNING */ => Level::Warn,
    };
    log!(
        level,
        "DebugCallback: {:?}: {:?}\n{} ({})\n{}\n",
        message_severity,
        message_type,
        message_id_name,
        message_id_number,
        message
    );

    #[cfg(feature = "validation_panic")]
    if level == Level::Error {
        panic!("Validation error");
    };

    vk::FALSE
}

pub struct Debugger {
    debug_utils: ext::DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT,
}

impl Debugger {
    pub fn new(entry: &ash::Entry, instance: &ash::Instance) -> VkResult<Self> {
        let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            )
            .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
            .pfn_user_callback(Some(vulkan_debug_callback));

        let debug_utils = ext::DebugUtils::new(entry, instance);
        let debug_messenger =
            unsafe { debug_utils.create_debug_utils_messenger(&debug_info, None)? };

        Ok(Self {
            debug_utils,
            debug_messenger,
        })
    }
}

impl Drop for Debugger {
    fn drop(&mut self) {
        unsafe {
            self.debug_utils
                .destroy_debug_utils_messenger(self.debug_messenger, None)
        }
    }
}
