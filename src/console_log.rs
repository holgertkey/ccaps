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

// Startup header for foreground mode: the layouts Caps Lock cycles through as
// (code, name), the index of the current one, the keys, and what the log lines mean.
// Ends with a rule that separates it from the log.
pub fn header(version: &str, layouts: &[(String, String)], current: Option<usize>) -> String {
    let mut lines = vec![
        format!(
            "CCaps Layout Switcher v{} · foreground mode · Ctrl+C to exit",
            version
        ),
        String::new(),
        "Layouts (Caps Lock cycles through them):".to_string(),
    ];
    if layouts.is_empty() {
        lines.push("  none found".to_string());
    }
    let name_width = layouts.iter().map(|(_, name)| name.chars().count()).max();
    for (i, (code, name)) in layouts.iter().enumerate() {
        let line = if current == Some(i) {
            format!(
                "  {:<3} {:<width$}  ← current",
                code,
                name,
                width = name_width.unwrap_or(0)
            )
        } else {
            format!("  {:<3} {}", code, name)
        };
        lines.push(line);
    }
    lines.extend(
        [
            "",
            "Shift + Caps Lock    toggle Caps Lock",
            "Scroll Lock LED      OFF = English, ON = another layout",
            "",
            "Log, one line per Caps Lock press:",
            "  Switched            the window applied the layout request",
        ]
        .map(String::from),
    );
    lines.push(format!(
        "  Switched late       applied after the {} ms check, no Win+Space needed",
        crate::layout_switcher::VERIFY_TIMEOUT_MS
    ));
    lines.extend(
        [
            "  SwitchedByFallback  the window ignored the request; reached with Win+Space",
            "  Unverified          can't tell whether the layout changed (see the reason)",
            "  Failed              the layout was not changed (see the reason)",
            "  'not switched by CCaps' lines: the layout changed another way, the LED followed",
            RULE,
        ]
        .map(String::from),
    );
    lines.join("\n")
}

const RULE: &str = "──────────────────────────────────────────────────────────────────────────────";

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

    fn layouts() -> Vec<(String, String)> {
        vec![
            ("us".to_string(), "English (United States)".to_string()),
            ("ru".to_string(), "Russian".to_string()),
        ]
    }

    #[test]
    fn test_header_lists_each_layout_once_and_marks_current() {
        let header = header("0.11.0", &layouts(), Some(0));
        assert!(header.starts_with("CCaps Layout Switcher v0.11.0 · foreground mode"));
        assert!(header.contains("\n  us  English (United States)  ← current\n"));
        assert!(header.contains("\n  ru  Russian\n"));
        assert_eq!(header.matches("Russian").count(), 1);
        assert_eq!(header.matches("← current").count(), 1);
        assert!(header.ends_with(RULE));
    }

    #[test]
    fn test_header_without_known_current_layout() {
        // The current layout isn't among the selected ones (or can't be read)
        let header = header("0.11.0", &layouts(), None);
        assert!(!header.contains("← current"));
    }

    #[test]
    fn test_header_aligns_current_marker_with_longest_name() {
        let header = header("0.11.0", &layouts(), Some(1));
        assert!(header.contains("\n  ru  Russian                  ← current\n"));
    }

    #[test]
    fn test_time_is_zero_padded() {
        assert_eq!(format_time(9, 5, 3, 7), "09:05:03.007");
        assert_eq!(format_time(23, 59, 59, 999), "23:59:59.999");
    }
}
