#[macro_export]
macro_rules! log {
    ($formatter:ident: $severity:expr, $($arg:tt)*) => {
        $formatter!(
            "[{}] {}: {}",
            chrono::Local::now().format("%H:%M:%S"),
            $severity,
			format!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! sev {
    (e) => {
        "error".red().bold()
    };
    (w) => {
        "warning".yellow().bold()
    };
    (i) => {
        "info".green().bold()
    };
    (d) => {
        "debug".cyan().bold()
    };
}

// error

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

// test

#[test]
fn test() {
    use colored::Colorize;
    log_error!("{}", 2);
}
