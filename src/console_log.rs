// Diagnostics printed to the console in foreground mode (ccaps -run): one line per
// Caps Lock press, and indicator changes CCaps didn't cause. Disabled in background mode,
// where there is no console.

use std::sync::atomic::{AtomicBool, Ordering};
use winapi::um::minwinbase::SYSTEMTIME;
use winapi::um::sysinfoapi::GetLocalTime;

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn enable() {
    ENABLED.store(true, Ordering::SeqCst);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

// Prints `message` with the local time, if diagnostics are enabled
pub fn log(message: &str) {
    if enabled() {
        println!("[{}] {}", timestamp(), message);
    }
}

// Explains the outcomes printed for every switch
pub fn print_legend() {
    println!("Diagnostics, one line per Caps Lock press:");
    println!("  Switched            - the window applied the layout request");
    println!("  Switched late       - applied after the check, no Win+Space needed");
    println!("  SwitchedByFallback  - the window ignored the request; reached with Win+Space");
    println!("  Unverified          - can't tell whether the layout changed (see the reason)");
    println!("  Failed              - the layout was not changed (see the reason)");
    println!("Lines with 'Scroll Lock' report layout changes not made by CCaps.");
    println!();
}

// Local time as HH:MM:SS.mmm
fn timestamp() -> String {
    let mut time: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut time) };
    format_time(time.wHour, time.wMinute, time.wSecond, time.wMilliseconds)
}

fn format_time(hour: u16, minute: u16, second: u16, millis: u16) -> String {
    format!("{:02}:{:02}:{:02}.{:03}", hour, minute, second, millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_is_zero_padded() {
        assert_eq!(format_time(9, 5, 3, 7), "09:05:03.007");
        assert_eq!(format_time(23, 59, 59, 999), "23:59:59.999");
    }
}
