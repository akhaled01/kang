use chrono::Local;

const INFO_PREFIX: &str = "\x1b[1;32m[INFO]\x1b[0m"; // Bold Green
const WARN_PREFIX: &str = "\x1b[1;33m[WARN]\x1b[0m"; // Bold Yellow
const ERROR_PREFIX: &str = "\x1b[1;31m[ERROR]\x1b[0m"; // Bold Red
const DEBUG_PREFIX: &str = "\x1b[1;36m[DEBUG]\x1b[0m"; // Bold Cyan

/// Logs an informational message with format args
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::logging::logger::_info(&format_args!($($arg)*).to_string())
    }
}

/// Internal info function called by the macro
pub fn _info(message: &str) {
    log(INFO_PREFIX, message);
}

/// Logs a warning message with format args
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::logging::logger::_warn(&format_args!($($arg)*).to_string())
    }
}

/// Internal warn function called by the macro
pub fn _warn(message: &str) {
    log(WARN_PREFIX, message);
}

/// Logs an error message with format args
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::logging::logger::_error(&format_args!($($arg)*).to_string())
    }
}

/// Internal error function called by the macro
pub fn _error(message: &str) {
    log(ERROR_PREFIX, message);
}

/// Logs a debug message with format args
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::logging::logger::_debug(&format_args!($($arg)*).to_string())
    }
}

/// Internal debug function called by the macro
pub fn _debug(message: &str) {
    log(DEBUG_PREFIX, message);
}

/// Internal logging function that handles the formatting
fn log(level: &str, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    println!("[{}] {} {}", timestamp, level, message);
}
