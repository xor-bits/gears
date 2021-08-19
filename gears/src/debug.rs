use vulkano::instance::debug::{Message, MessageSeverity, MessageType};

pub const SEVERITY: MessageSeverity = MessageSeverity {
    error: true,
    warning: true,
    information: true,
    verbose: false,
};

pub const TY: MessageType = MessageType {
    general: true,
    performance: true,
    validation: true,
};

pub fn callback(message: &Message) {
    let level = if message.severity.error {
        log::Level::Error
    } else if message.severity.warning {
        log::Level::Warn
    } else if message.severity.information {
        log::Level::Info
    } else {
        log::Level::Trace
    };

    log::log!(level, "DebugCallback: \n{}", message.description);

    #[cfg(feature = "validation_panic")]
    if level == log::Level::Error {
        panic!("Validation error");
    }
}
