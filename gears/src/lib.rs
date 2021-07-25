pub mod context;
mod debug;
pub mod format;
pub mod frame;
pub mod io;
pub mod loops;
pub mod renderer;

#[cfg(feature = "short_namespaces")]
pub use context::*;
#[cfg(feature = "short_namespaces")]
pub use format::*;
#[cfg(feature = "short_namespaces")]
pub use frame::*;
#[cfg(feature = "short_namespaces")]
pub use io::*;
#[cfg(feature = "short_namespaces")]
pub use loops::*;
#[cfg(feature = "short_namespaces")]
pub use renderer::*;

#[cfg(feature = "runtime_shaders")]
pub use gears_spirv;

pub use ash::vk;
pub use gears_pipeline::*;
pub use glam;
pub use static_assertions;

use log::error;
use std::{fmt, time};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum SyncMode {
    /// Immediate: no sync
    ///
    /// Pros:
    /// + Minimal input delay
    ///
    /// Cons:
    /// - Will result screen tearing
    /// - Consumes more power
    /// - Might not be supported (unlikely) (fallback to Fifo)
    Immediate,

    /// FIFO: sync with no discards (VSync)
    ///
    /// Pros:
    /// + Eliminates screen tearing
    /// + Consumes less power
    /// + Always supported
    ///
    /// Cons:
    /// - Increased input delay
    Fifo,

    /// Mailbox: sync with discards
    ///
    /// Pros:
    /// + Eliminates screen tearing
    /// + Minimal input delay
    ///
    /// Cons:
    /// - Consumes more power
    /// - Might not be supported (fallback to Fifo)
    Mailbox,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum UpdateRate {
    /// _n_ updates per second with even intervals
    /// Ex: Update 60 times every second = ```UpdateRate::PerSecond::(60)```
    PerSecond(u32),

    /// _n_ updates per minute with even intervals
    /// Ex: Update 2 times every minute = ```UpdateRate::PerMinute::(2)```
    PerMinute(u32),

    /// _t_ update interval
    /// Ex: Update every 2 seconds = ```UpdateRate::Interval::(Duration::from_secs(2))```
    Interval(time::Duration),
}

impl Default for SyncMode {
    fn default() -> Self {
        SyncMode::Fifo
    }
}

impl UpdateRate {
    pub fn to_interval(&self) -> time::Duration {
        match *self {
            UpdateRate::PerSecond(n) => time::Duration::from_secs_f64(1.0).div_f64(n as f64),
            UpdateRate::PerMinute(n) => time::Duration::from_secs_f64(60.0).div_f64(n as f64),
            UpdateRate::Interval(i) => i,
        }
    }
}

// Internal helper traits:

trait ExpectLog<T> {
    fn expect_log<'a, S: Into<&'a str>>(self, message: S) -> T;
}

impl<T> ExpectLog<T> for Option<T> {
    fn expect_log<'a, S: Into<&'a str>>(self, message: S) -> T {
        self.unwrap_or_else(|| {
            error!("{}", message.into());
            panic!();
        })
    }
}

impl<T, E: fmt::Debug> ExpectLog<T> for Result<T, E> {
    fn expect_log<'a, S: Into<&'a str>>(self, message: S) -> T {
        self.unwrap_or_else(|err| {
            error!("{}: {:?}", message.into(), err);
            panic!();
        })
    }
}

trait MapErrorLog<T, E> {
    fn map_err_log<'a, S: Into<&'a str>>(self, message: S, or: E) -> Result<T, E>;
}

impl<T, E> MapErrorLog<T, E> for Option<T> {
    fn map_err_log<'a, S: Into<&'a str>>(self, message: S, or: E) -> Result<T, E> {
        self.ok_or_else(|| {
            error!("{}", message.into());
            or
        })
    }
}

impl<T, Ea: fmt::Debug, Eb> MapErrorLog<T, Eb> for Result<T, Ea> {
    fn map_err_log<'a, S: Into<&'a str>>(self, message: S, or: Eb) -> Result<T, Eb> {
        self.map_err(|err| {
            error!("{}: {:?}", message.into(), err);
            or
        })
    }
}

trait MapErrorElseLogOption<T, E> {
    fn map_err_else_log<'a, S: Into<&'a str>, F: Fn() -> E>(
        self,
        message: S,
        or: F,
    ) -> Result<T, E>;
}

trait MapErrorElseLogResult<T, Ea, Eb> {
    fn map_err_else_log<'a, S: Into<&'a str>, F: Fn(Ea) -> Eb>(
        self,
        message: S,
        or: F,
    ) -> Result<T, Eb>;
}

impl<T, E> MapErrorElseLogOption<T, E> for Option<T> {
    fn map_err_else_log<'a, S: Into<&'a str>, F: Fn() -> E>(
        self,
        message: S,
        or: F,
    ) -> Result<T, E> {
        self.ok_or_else(|| {
            error!("{}", message.into());
            or()
        })
    }
}

impl<T, Ea: fmt::Debug, Eb> MapErrorElseLogResult<T, Ea, Eb> for Result<T, Ea> {
    fn map_err_else_log<'a, S: Into<&'a str>, F: Fn(Ea) -> Eb>(
        self,
        message: S,
        or: F,
    ) -> Result<T, Eb> {
        self.map_err(|err| {
            error!("{}: {:?}", message.into(), err);
            or(err)
        })
    }
}
