use crate::keyboard_hook::CCAPS_EXTRA_INFO;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use winapi::shared::basetsd::UINT_PTR;
use winapi::shared::minwindef::{DWORD, HKL, UINT};
use winapi::shared::windef::HWND;
use winapi::um::winuser::*;

// Function to check if given layout is English
unsafe fn is_english_layout_hkl(layout: HKL) -> bool {
    // Extract language ID from layout handle
    // Lower 16 bits contain the language identifier
    let lang_id = (layout as usize) & 0xFFFF;

    // English language IDs:
    // 0x0409 - English (United States)
    // 0x0809 - English (United Kingdom)
    // 0x0c09 - English (Australia)
    // 0x1009 - English (Canada)
    // 0x1409 - English (New Zealand)
    // 0x1809 - English (Ireland)
    // 0x1c09 - English (South Africa)
    // 0x2009 - English (Jamaica)
    // 0x2409 - English (Caribbean)
    // 0x2809 - English (Belize)
    // 0x2c09 - English (Trinidad)
    // 0x3009 - English (Zimbabwe)
    // 0x3409 - English (Philippines)
    matches!(
        lang_id,
        0x0409
            | 0x0809
            | 0x0c09
            | 0x1009
            | 0x1409
            | 0x1809
            | 0x1c09
            | 0x2009
            | 0x2409
            | 0x2809
            | 0x2c09
            | 0x3009
            | 0x3409
    )
}

// Function to set Scroll Lock state
unsafe fn set_scroll_lock_state(enabled: bool) {
    unsafe {
        // Get current Scroll Lock state
        let current_state = GetKeyState(VK_SCROLL) & 1;
        let is_currently_on = current_state != 0;

        // Only change if state is different
        if is_currently_on != enabled {
            // Create input for toggling Scroll Lock
            let mut inputs: [INPUT; 2] = mem::zeroed();

            // First INPUT - Scroll Lock press
            inputs[0].type_ = INPUT_KEYBOARD;
            inputs[0].u.ki_mut().wVk = VK_SCROLL as u16;
            inputs[0].u.ki_mut().dwFlags = 0;

            // Second INPUT - Scroll Lock release
            inputs[1].type_ = INPUT_KEYBOARD;
            inputs[1].u.ki_mut().wVk = VK_SCROLL as u16;
            inputs[1].u.ki_mut().dwFlags = KEYEVENTF_KEYUP;

            // Send press and release events
            SendInput(2, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as i32);
        }
    }
}

// Function to check if current layout is English
unsafe fn is_english_layout() -> bool {
    unsafe {
        // Get active window
        let hwnd = GetForegroundWindow();

        // Get current layout handle
        let current_layout = if hwnd.is_null() {
            // Fallback: get layout for current thread if no foreground window
            // This is more reliable during program startup
            GetKeyboardLayout(0)
        } else {
            // Get window thread ID and its layout
            let thread_id = GetWindowThreadProcessId(hwnd, ptr::null_mut());
            GetKeyboardLayout(thread_id)
        };

        if current_layout.is_null() {
            return false;
        }

        is_english_layout_hkl(current_layout)
    }
}

// Scroll Lock state CCaps last applied: INDICATOR_UNKNOWN, INDICATOR_OFF or INDICATOR_ON
const INDICATOR_UNKNOWN: u8 = 0;
const INDICATOR_OFF: u8 = 1;
const INDICATOR_ON: u8 = 2;
static LAST_INDICATOR: AtomicU8 = AtomicU8::new(INDICATOR_UNKNOWN);

// When CCaps last requested a layout switch; the periodic sync leaves the indicator
// alone for SWITCH_GRACE after it, while the foreground window applies the request
static LAST_SWITCH: Mutex<Option<Instant>> = Mutex::new(None);
const SWITCH_GRACE: Duration = Duration::from_millis(500);

// How often the indicator is checked against the foreground window's layout
const SYNC_INTERVAL_MS: u32 = 250;

// How much later than SYNC_INTERVAL_MS Windows may fire the timer, so that it can
// coalesce the wake-up with other timers instead of waking the CPU just for CCaps
const SYNC_TOLERANCE_MS: u32 = 100;

// Sets Scroll Lock to `enabled` and remembers it as the last applied state
unsafe fn apply_indicator(enabled: bool) {
    unsafe {
        set_scroll_lock_state(enabled);
        LAST_INDICATOR.store(
            if enabled { INDICATOR_ON } else { INDICATOR_OFF },
            Ordering::SeqCst,
        );
    }
}

// Decides whether the periodic sync must change Scroll Lock: only when the wanted
// state differs from the one CCaps last applied, and not during the grace period after
// a switch. Comparing with the last applied state (not the live key state) leaves a
// manual Scroll Lock press alone until the layout changes again.
fn sync_action(last_applied: u8, wanted_on: bool, in_grace: bool) -> Option<bool> {
    if in_grace {
        return None;
    }
    let wanted = if wanted_on {
        INDICATOR_ON
    } else {
        INDICATOR_OFF
    };
    if last_applied == wanted {
        None
    } else {
        Some(wanted_on)
    }
}

// Public function to update Scroll Lock indicator with specific layout.
// Called right after CCaps requested a switch to `layout`: shows the new state at once
// and starts the grace period during which the periodic sync doesn't override it.
pub unsafe fn update_layout_indicator_with_layout(layout: HKL) {
    unsafe {
        if let Ok(mut last_switch) = LAST_SWITCH.lock() {
            *last_switch = Some(Instant::now());
        }

        // English layout: Scroll Lock OFF
        // Non-English layout: Scroll Lock ON
        apply_indicator(!is_english_layout_hkl(layout));
    }
}

// Public function to update Scroll Lock indicator based on current layout
pub unsafe fn update_layout_indicator() {
    unsafe {
        // English layout: Scroll Lock OFF
        // Non-English layout: Scroll Lock ON
        apply_indicator(!is_english_layout());
    }
}

// Timer callback: keeps Scroll Lock in step with the foreground window's actual layout,
// which can change without CCaps (Win+Space, focusing a window that has another layout,
// a window that ignored CCaps's switch request)
unsafe extern "system" fn sync_timer_proc(_hwnd: HWND, _msg: UINT, _id: UINT_PTR, _time: DWORD) {
    unsafe {
        let in_grace = LAST_SWITCH
            .lock()
            .ok()
            .and_then(|last_switch| *last_switch)
            .is_some_and(|at| at.elapsed() < SWITCH_GRACE);
        let last_applied = LAST_INDICATOR.load(Ordering::SeqCst);

        if let Some(enabled) = sync_action(last_applied, !is_english_layout(), in_grace) {
            apply_indicator(enabled);
        }
    }
}

// Public function to start keeping the indicator in sync. Uses a thread timer, so it
// must be called on the thread that runs the message loop.
pub unsafe fn start_indicator_sync() {
    unsafe {
        let timer = SetCoalescableTimer(
            ptr::null_mut(),
            0,
            SYNC_INTERVAL_MS,
            Some(sync_timer_proc),
            SYNC_TOLERANCE_MS,
        );
        if timer == 0 {
            // Coalescing unavailable: fall back to a plain timer
            SetTimer(ptr::null_mut(), 0, SYNC_INTERVAL_MS, Some(sync_timer_proc));
        }
    }
}

// Public function to ensure CapsLock is turned off at startup.
// Since CCaps repurposes the CapsLock key for layout switching,
// CapsLock should be off when the program starts.
// Only sends key events if CapsLock is actually on, avoiding
// unnecessary toggles that can cause LED desynchronization during
// Windows startup due to unreliable SendInput timing.
pub unsafe fn ensure_caps_lock_off() {
    unsafe {
        // Synchronize the thread's key state table before querying.
        // GetKeyState() relies on the thread's internal key state buffer,
        // which may be stale during Windows startup before the message loop runs.
        // GetKeyboardState() forces a synchronization with the actual key states.
        let mut key_state: [u8; 256] = [0; 256];
        GetKeyboardState(key_state.as_mut_ptr());

        let caps_state = GetKeyState(VK_CAPITAL) & 1;
        if caps_state != 0 {
            // CapsLock is on, toggle it off with a single press+release
            let mut inputs: [INPUT; 2] = std::mem::zeroed();

            inputs[0].type_ = INPUT_KEYBOARD;
            inputs[0].u.ki_mut().wVk = VK_CAPITAL as u16;
            inputs[0].u.ki_mut().dwFlags = 0;
            inputs[0].u.ki_mut().dwExtraInfo = CCAPS_EXTRA_INFO;

            inputs[1].type_ = INPUT_KEYBOARD;
            inputs[1].u.ki_mut().wVk = VK_CAPITAL as u16;
            inputs[1].u.ki_mut().dwFlags = KEYEVENTF_KEYUP;
            inputs[1].u.ki_mut().dwExtraInfo = CCAPS_EXTRA_INFO;

            SendInput(2, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
        }
    }
}

// Public function to get current layout information (for debugging)
#[allow(dead_code)]
pub unsafe fn get_current_layout_info() -> (String, bool) {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return ("Unknown".to_string(), false);
        }

        let thread_id = GetWindowThreadProcessId(hwnd, ptr::null_mut());
        let current_layout = GetKeyboardLayout(thread_id);
        let lang_id = (current_layout as usize) & 0xFFFF;
        let is_english = is_english_layout_hkl(current_layout);

        let layout_name = match lang_id {
            0x0409 => "English (US)",
            0x0809 => "English (UK)",
            0x0419 => "Russian",
            0x0422 => "Ukrainian",
            0x0407 => "German",
            0x040C => "French",
            0x0410 => "Italian",
            0x040A => "Spanish",
            0x0415 => "Polish",
            _ => "Other",
        };

        (format!("{} (0x{:04X})", layout_name, lang_id), is_english)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_turns_indicator_on_when_layout_became_non_english() {
        assert_eq!(sync_action(INDICATOR_OFF, true, false), Some(true));
    }

    #[test]
    fn test_sync_turns_indicator_off_when_layout_became_english() {
        assert_eq!(sync_action(INDICATOR_ON, false, false), Some(false));
    }

    #[test]
    fn test_sync_applies_state_when_nothing_was_applied_yet() {
        assert_eq!(sync_action(INDICATOR_UNKNOWN, false, false), Some(false));
        assert_eq!(sync_action(INDICATOR_UNKNOWN, true, false), Some(true));
    }

    #[test]
    fn test_sync_does_nothing_when_state_already_matches() {
        // Leaves a manual Scroll Lock press alone until the layout changes again
        assert_eq!(sync_action(INDICATOR_ON, true, false), None);
        assert_eq!(sync_action(INDICATOR_OFF, false, false), None);
    }

    #[test]
    fn test_sync_waits_during_grace_period_after_switch() {
        // The foreground window may not have applied the switch request yet
        assert_eq!(sync_action(INDICATOR_ON, false, true), None);
    }

    // Helper function to create a fake HKL from a language ID
    fn create_test_hkl(lang_id: usize) -> HKL {
        lang_id as HKL
    }

    #[test]
    fn test_english_us_layout_detection() {
        unsafe {
            let hkl = create_test_hkl(0x0409);
            assert!(
                is_english_layout_hkl(hkl),
                "English (US) should be detected as English"
            );
        }
    }

    #[test]
    fn test_english_uk_layout_detection() {
        unsafe {
            let hkl = create_test_hkl(0x0809);
            assert!(
                is_english_layout_hkl(hkl),
                "English (UK) should be detected as English"
            );
        }
    }

    #[test]
    fn test_english_variants_detection() {
        unsafe {
            // Test various English language variants
            let english_variants = vec![
                (0x0409, "US"),
                (0x0809, "UK"),
                (0x0c09, "Australia"),
                (0x1009, "Canada"),
                (0x1409, "New Zealand"),
                (0x1809, "Ireland"),
                (0x1c09, "South Africa"),
            ];

            for (lang_id, country) in english_variants {
                let hkl = create_test_hkl(lang_id);
                assert!(
                    is_english_layout_hkl(hkl),
                    "English ({}) layout 0x{:04X} should be detected as English",
                    country,
                    lang_id
                );
            }
        }
    }

    #[test]
    fn test_non_english_layout_detection() {
        unsafe {
            // Test that non-English layouts are correctly identified
            let non_english_layouts = vec![
                (0x0419, "Russian"),
                (0x0407, "German"),
                (0x040c, "French"),
                (0x0410, "Italian"),
                (0x040a, "Spanish"),
                (0x0415, "Polish"),
                (0x0422, "Ukrainian"),
            ];

            for (lang_id, language) in non_english_layouts {
                let hkl = create_test_hkl(lang_id);
                assert!(
                    !is_english_layout_hkl(hkl),
                    "{} layout 0x{:04X} should NOT be detected as English",
                    language,
                    lang_id
                );
            }
        }
    }

    #[test]
    fn test_layout_hkl_language_id_extraction() {
        unsafe {
            // Test that language ID is correctly extracted from HKL
            // HKL format: lower 16 bits = language ID, upper 16 bits = device handle

            // Create HKL with device handle in upper bits
            let lang_id = 0x0409; // English (US)
            let device_handle = 0xABCD;
            let hkl = ((device_handle << 16) | lang_id) as HKL;

            // Language ID should still be correctly extracted
            let extracted_lang_id = (hkl as usize) & 0xFFFF;
            assert_eq!(
                extracted_lang_id, lang_id,
                "Language ID should be correctly extracted from HKL"
            );
            assert!(
                is_english_layout_hkl(hkl),
                "English layout should be detected even with device handle"
            );
        }
    }

    #[test]
    fn test_zero_hkl() {
        unsafe {
            let hkl = create_test_hkl(0x0000);
            assert!(
                !is_english_layout_hkl(hkl),
                "Zero HKL should not be detected as English"
            );
        }
    }
}
