pub mod context;
mod debug;
pub mod frame;
pub mod io;
pub mod loops;
pub mod renderer;

use log::error;
use std::{fmt, time};

#[cfg(feature = "short_namespaces")]
pub use context::*;
#[cfg(feature = "short_namespaces")]
pub use frame::*;
#[cfg(feature = "short_namespaces")]
pub use io::*;
#[cfg(feature = "short_namespaces")]
pub use loops::*;
#[cfg(feature = "short_namespaces")]
pub use renderer::*;

#[derive(Debug)]
pub enum VSync {
    Off,
    On,
}

#[derive(Debug)]
pub enum UpdateRate {
    PerSecond(u32),
    PerMinute(u32),
    Interval(time::Duration),
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
