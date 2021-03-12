use std::fmt::Debug;

#[macro_export]
macro_rules! log {
    ($formatter:ident: $severity:expr, $($arg:tt)*) => {
        $formatter!(
            "[{}] {}: {}",
            /* chrono::Local::now().format("%H:%M:%S") */0,
            $severity,
			format!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! sev {
    (e) => {{
        use colored::Colorize;
        "error".red().bold()
    }};
    (w) => {{
        use colored::Colorize;
        "warning".yellow().bold()
    }};
    (i) => {{
        use colored::Colorize;
        "info".green().bold()
    }};
    (d) => {{
        use colored::Colorize;
        "debug".cyan().bold()
    }};
}

// error
#[macro_export]
macro_rules! log_option {
	($wrapped:expr, $($arg:tt)*) => {{
		$wrapped.unwrap_or_else(|| {
			log!(println: sev!(e), $($arg)*);
			println!();
			panic!(sev!(e))
		})
	}};
}

#[macro_export]
macro_rules! log_result {
	($wrapped:expr, $($arg:tt)*) => {{
		$wrapped.unwrap_or_else(|err| {
			log!(println: sev!(e), $($arg)*);
			println!();
			panic!("{}: {}", sev!(e), err)
		})
	}};
}

#[macro_export]
macro_rules! log_error {
	($($arg:tt)*) => {{
		log!(println: sev!(e), $($arg)*);
		println!();
		panic!(sev!(e))
	}};
}

#[macro_export]
macro_rules! log_error_string {
	($($arg:tt)*) => {
		log!(format: sev!(e), $($arg)*)
	}
}

#[macro_export]
macro_rules! log_error_write {
	($writter:expr, $($arg:tt)*) => {
		log!(write: sev!(e), $($arg)*)
	}
}

#[macro_export]
macro_rules! log_assert {
	($assure:expr, $($arg:tt)*) => {
		if !$assure {
			log!(panic: sev!(e), $($arg)*)
		}
	}
}

// warn

#[macro_export]
macro_rules! log_warn {
	($($arg:tt)*) => {
		log!(println: sev!(w), $($arg)*)
	}
}

#[macro_export]
macro_rules! log_warn_string {
	($($arg:tt)*) => {
		log!(format: sev!(w), $($arg)*)
	}
}

#[macro_export]
macro_rules! log_warn_write {
	($writter:expr, $($arg:tt)*) => {
		log!(write: sev!(w), $($arg)*)
	}
}

// info

#[macro_export]
macro_rules! log_info {
	($($arg:tt)*) => {
		log!(println: sev!(i), $($arg)*)
	}
}

#[macro_export]
macro_rules! log_info_string {
	($($arg:tt)*) => {
		log!(format: sev!(i), $($arg)*)
	}
}

#[macro_export]
macro_rules! log_info_write {
	($writter:expr, $($arg:tt)*) => {
		log!(write: sev!(i), $($arg)*)
	}
}

// debug

#[macro_export]
macro_rules! log_debug {
	($($arg:tt)*) => {
		log!(println: sev!(d), $($arg)*)
	}
}

#[macro_export]
macro_rules! log_debug_string {
	($($arg:tt)*) => {
		log!(format: sev!(d), $($arg)*)
	}
}

#[macro_export]
macro_rules! log_debug_write {
	($writter:expr, $($arg:tt)*) => {
		log!(write:sev!(d), $($arg)*)
	}
}

pub trait LogWrap<T> {
    fn expect_log(self, message: &str) -> T;
    fn unwrap_log(self) -> T;
}

impl<T> LogWrap<T> for Option<T> {
    fn expect_log(self, message: &str) -> T {
        self.unwrap_or_else(|| log_error!("{}", message))
    }

    fn unwrap_log(self) -> T {
        self.unwrap_or_else(|| log_error!("Failed to unwrap"))
    }
}

impl<T, E: Debug> LogWrap<T> for Result<T, E> {
    fn expect_log(self, message: &str) -> T {
        self.unwrap_or_else(|err| log_error!("{}: {:?}", message, err))
    }

    fn unwrap_log(self) -> T {
        self.unwrap_or_else(|err| log_error!("Failed to unwrap: {:?}", err))
    }
}

// test

#[test]
fn test() {
    log_error!("{}", 2);
}
