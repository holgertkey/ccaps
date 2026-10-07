use std::mem;
use std::ptr;
use winapi::shared::minwindef::{FALSE, HKL, LPARAM};
use winapi::shared::windef::HWND;
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::winbase::QueryFullProcessImageNameW;
use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;
use winapi::um::winuser::*;

#[derive(Debug, Clone)]
pub struct LayoutInfo {
    pub hkl: usize, // Changed from HKL to usize for Send + Sync
    #[allow(dead_code)]
    pub lang_id: u32,
    pub name: String,
    pub short_code: String,
    pub is_english: bool,
}

impl LayoutInfo {
    pub fn new(hkl: HKL) -> Self {
        let lang_id = (hkl as usize) & 0xFFFF;
        let (name, short_code, is_english) = get_layout_details(lang_id as u32);

        LayoutInfo {
            hkl: hkl as usize, // Convert HKL to usize
            lang_id: lang_id as u32,
            name,
            short_code,
            is_english,
        }
    }

    pub fn get_hkl(&self) -> HKL {
        self.hkl as HKL // Convert back to HKL when needed
    }
}

pub fn get_all_keyboard_layouts() -> Vec<LayoutInfo> {
    unsafe {
        let mut layouts: [HKL; 20] = mem::zeroed();
        let layout_count = GetKeyboardLayoutList(20, layouts.as_mut_ptr());

        let mut layout_infos: Vec<LayoutInfo> = layouts
            .iter()
            .take(layout_count as usize)
            .map(|&hkl| LayoutInfo::new(hkl))
            .collect();

        // Sort layouts: English first, then alphabetically
        layout_infos.sort_by(|a, b| match (a.is_english, b.is_english) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        layout_infos
    }
}

#[link(name = "imm32")]
extern "system" {
    fn ImmGetDefaultIMEWnd(hwnd: HWND) -> HWND;
}

// Layout to show at startup and in status output. Without a foreground window (e.g.
// right after login) falls back to CCaps's own thread layout, which is the default
// input language then. Not for later decisions: that thread's layout never changes.
pub fn get_current_layout() -> Option<LayoutInfo> {
    current_layout_hkl()
        .map(|hkl| LayoutInfo::new(hkl as HKL))
        .or_else(get_current_thread_layout)
}

// Layout the user is typing with right now, or None if it cannot be read: no foreground
// window, or a window whose input threads CCaps can't query
pub fn current_layout_hkl() -> Option<usize> {
    unsafe { foreground_layout() }
}

// Reads the layout of the foreground window from the threads that may own its input,
// best first:
// - the thread of the focused control: in Windows 11 Notepad and UWP apps it differs
//   from the top-level window's thread, whose layout goes stale after Win+Space;
// - the thread of the foreground window itself;
// - the thread of its default IME window: for console windows both threads above read
//   as 0, while the IME window belongs to the real conhost input thread.
unsafe fn foreground_layout() -> Option<usize> {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return None;
        }

        let mut info: GUITHREADINFO = mem::zeroed();
        info.cbSize = mem::size_of::<GUITHREADINFO>() as u32;
        let focus_thread = if GetGUIThreadInfo(0, &mut info) != 0 {
            window_thread(info.hwndFocus)
        } else {
            0
        };

        first_readable_layout(&[
            &|| thread_layout(focus_thread),
            &|| thread_layout(window_thread(foreground)),
            &|| thread_layout(window_thread(ImmGetDefaultIMEWnd(foreground))),
        ])
    }
}

// Thread that owns `hwnd`, or 0 if there is no window
unsafe fn window_thread(hwnd: HWND) -> u32 {
    unsafe {
        if hwnd.is_null() {
            0
        } else {
            GetWindowThreadProcessId(hwnd, ptr::null_mut())
        }
    }
}

// Keyboard layout of `thread`, or 0 if unknown. Thread 0 must not reach
// GetKeyboardLayout: it would return CCaps's own layout instead of "unknown".
fn thread_layout(thread: u32) -> usize {
    if thread == 0 {
        0
    } else {
        unsafe { GetKeyboardLayout(thread) as usize }
    }
}

// First non-zero layout from `sources`, evaluated lazily in order
fn first_readable_layout(sources: &[&dyn Fn() -> usize]) -> Option<usize> {
    sources.iter().map(|source| source()).find(|&hkl| hkl != 0)
}

pub fn get_current_thread_layout() -> Option<LayoutInfo> {
    unsafe {
        // Get keyboard layout for the current thread (0 = calling thread)
        // This works reliably even at startup when there's no foreground window
        let current_layout = GetKeyboardLayout(0);
        if current_layout.is_null() {
            return None;
        }

        Some(LayoutInfo::new(current_layout))
    }
}

pub fn find_layouts_by_codes(codes: &[&str]) -> Vec<LayoutInfo> {
    let all_layouts = get_all_keyboard_layouts();
    let mut selected_layouts = Vec::new();

    for code in codes {
        if let Some(layout) = all_layouts.iter().find(|l| l.short_code == *code) {
            selected_layouts.push(layout.clone());
        }
    }

    selected_layouts
}

pub fn get_english_layout() -> Option<LayoutInfo> {
    let all_layouts = get_all_keyboard_layouts();
    all_layouts.into_iter().find(|l| l.is_english)
}

// Asks the foreground window to switch to `hkl`. Returns false if there is no window to
// ask or the request was rejected: PostMessageW fails with ERROR_ACCESS_DENIED for
// elevated windows. True doesn't mean the window applied it: some windows ignore the
// request, so the result must be verified by reading the layout.
//
// CCaps doesn't call ActivateKeyboardLayout: it only changes CCaps's own thread layout.
pub fn post_layout_request(hkl: usize) -> bool {
    unsafe {
        // Ask the focused window of the foreground thread to change its layout. This used
        // to be posted to HWND_BROADCAST, which also reached hidden top-level windows of
        // other threads (e.g. an OpenGL driver's helper windows) and could deadlock such
        // processes when two of their threads handled the layout change at the same time.
        // Posting to the top-level window alone is not enough either: dialogs such as the
        // Explorer "Save As" dialog ignore the request unless it reaches the focused control.
        match foreground_request_target() {
            Some(target) => PostMessageW(target, WM_INPUTLANGCHANGEREQUEST, 0, hkl as LPARAM) != 0,
            None => false,
        }
    }
}

// Whether the foreground window processes messages within `timeout_ms`. A busy window
// keeps a posted layout request in its queue and applies it later.
pub fn foreground_responsive(timeout_ms: u32) -> bool {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() || IsHungAppWindow(foreground) != 0 {
            return false;
        }
        let Some(target) = foreground_request_target() else {
            return false;
        };
        let mut result = 0;
        SendMessageTimeoutW(
            target,
            WM_NULL,
            0,
            0,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            timeout_ms,
            &mut result,
        ) != 0
    }
}

// Foreground application for diagnostics: "exe (window class)", e.g.
// "Code.exe (Chrome_WidgetWin_1)"
pub fn foreground_app() -> String {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return "no foreground window".to_string();
        }

        let mut class = [0u16; 128];
        let class_len = GetClassNameW(foreground, class.as_mut_ptr(), class.len() as i32);
        let class = String::from_utf16_lossy(&class[..class_len.max(0) as usize]);

        let mut pid = 0;
        GetWindowThreadProcessId(foreground, &mut pid);
        format!("{} ({})", process_exe_name(pid), class)
    }
}

// File name of the executable of process `pid`, "?" if it can't be read (e.g. a
// protected process)
unsafe fn process_exe_name(pid: u32) -> String {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
        if process.is_null() {
            return "?".to_string();
        }
        let mut path = [0u16; 512];
        let mut len = path.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len);
        CloseHandle(process);
        if ok == 0 {
            return "?".to_string();
        }
        exe_file_name(&String::from_utf16_lossy(&path[..len as usize])).to_string()
    }
}

// "C:\Program Files\App\app.exe" -> "app.exe"
fn exe_file_name(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or(path)
}

// Installed layouts (HKLs), in the system's order
pub fn installed_layouts() -> Vec<usize> {
    unsafe {
        let mut layouts: [HKL; 32] = mem::zeroed();
        let count = GetKeyboardLayoutList(layouts.len() as i32, layouts.as_mut_ptr());
        layouts
            .iter()
            .take(count.max(0) as usize)
            .map(|&hkl| hkl as usize)
            .collect()
    }
}

// Window that should receive WM_INPUTLANGCHANGEREQUEST for the foreground window
unsafe fn foreground_request_target() -> Option<HWND> {
    unsafe {
        let foreground = GetForegroundWindow();
        layout_request_target(foreground, focused_window(foreground))
    }
}

// Returns the window that has keyboard focus in the thread owning `foreground`,
// or null if it cannot be determined.
unsafe fn focused_window(foreground: HWND) -> HWND {
    unsafe {
        if foreground.is_null() {
            return ptr::null_mut();
        }
        let thread_id = GetWindowThreadProcessId(foreground, ptr::null_mut());
        if thread_id == 0 {
            return ptr::null_mut();
        }

        let mut info: GUITHREADINFO = mem::zeroed();
        info.cbSize = mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(thread_id, &mut info) == 0 {
            return ptr::null_mut();
        }

        info.hwndFocus
    }
}

// Returns the window that should receive WM_INPUTLANGCHANGEREQUEST: the focused window
// of the foreground thread, or the foreground window itself if nothing has focus.
// Never a broadcast.
fn layout_request_target(foreground: HWND, focused: HWND) -> Option<HWND> {
    if foreground.is_null() || foreground == HWND_BROADCAST {
        None
    } else if focused.is_null() || focused == HWND_BROADCAST {
        Some(foreground)
    } else {
        Some(focused)
    }
}

fn get_layout_details(lang_id: u32) -> (String, String, bool) {
    match lang_id {
        // English variants
        0x0409 => (
            "English (United States)".to_string(),
            "us".to_string(),
            true,
        ),
        0x0809 => (
            "English (United Kingdom)".to_string(),
            "gb".to_string(),
            true,
        ),
        0x0c09 => ("English (Australia)".to_string(), "au".to_string(), true),
        0x1009 => ("English (Canada)".to_string(), "ca".to_string(), true),
        0x1409 => ("English (New Zealand)".to_string(), "nz".to_string(), true),
        0x1809 => ("English (Ireland)".to_string(), "ie".to_string(), true),
        0x1c09 => ("English (South Africa)".to_string(), "za".to_string(), true),

        // Cyrillic languages
        0x0419 => ("Russian".to_string(), "ru".to_string(), false),
        0x0422 => ("Ukrainian".to_string(), "ua".to_string(), false),
        0x0423 => ("Belarusian".to_string(), "by".to_string(), false),
        0x0402 => ("Bulgarian".to_string(), "bg".to_string(), false),
        0x041a => ("Croatian".to_string(), "hr".to_string(), false),
        0x0405 => ("Czech".to_string(), "cz".to_string(), false),
        0x081a => ("Serbian (Latin)".to_string(), "rs".to_string(), false),
        0x0c1a => ("Serbian (Cyrillic)".to_string(), "sr".to_string(), false),
        0x041f => ("Turkish".to_string(), "tr".to_string(), false),

        // Western European
        0x0407 => ("German".to_string(), "de".to_string(), false),
        0x040c => ("French".to_string(), "fr".to_string(), false),
        0x0410 => ("Italian".to_string(), "it".to_string(), false),
        0x040a => ("Spanish".to_string(), "es".to_string(), false),
        0x0413 => ("Dutch".to_string(), "nl".to_string(), false),
        0x0414 => ("Norwegian".to_string(), "no".to_string(), false),
        0x041d => ("Swedish".to_string(), "se".to_string(), false),
        0x0406 => ("Danish".to_string(), "dk".to_string(), false),
        0x040b => ("Finnish".to_string(), "fi".to_string(), false),
        0x0816 => ("Portuguese".to_string(), "pt".to_string(), false),
        0x0416 => ("Portuguese (Brazil)".to_string(), "br".to_string(), false),

        // Eastern European
        0x0415 => ("Polish".to_string(), "pl".to_string(), false),
        0x040e => ("Hungarian".to_string(), "hu".to_string(), false),
        0x0418 => ("Romanian".to_string(), "ro".to_string(), false),
        0x041b => ("Slovak".to_string(), "sk".to_string(), false),
        0x0424 => ("Slovenian".to_string(), "si".to_string(), false),
        0x0425 => ("Estonian".to_string(), "ee".to_string(), false),
        0x0426 => ("Latvian".to_string(), "lv".to_string(), false),
        0x0427 => ("Lithuanian".to_string(), "lt".to_string(), false),

        // Asian languages
        0x0411 => ("Japanese".to_string(), "jp".to_string(), false),
        0x0412 => ("Korean".to_string(), "kr".to_string(), false),
        0x0404 => ("Chinese (Traditional)".to_string(), "tw".to_string(), false),
        0x0804 => ("Chinese (Simplified)".to_string(), "cn".to_string(), false),
        0x041e => ("Thai".to_string(), "th".to_string(), false),
        0x042a => ("Vietnamese".to_string(), "vn".to_string(), false),

        // Middle Eastern
        0x040d => ("Hebrew".to_string(), "he".to_string(), false),
        0x0401 => ("Arabic".to_string(), "ar".to_string(), false),
        0x0429 => ("Farsi".to_string(), "fa".to_string(), false),

        // Other
        0x040f => ("Icelandic".to_string(), "is".to_string(), false),
        0x0408 => ("Greek".to_string(), "gr".to_string(), false),
        0x041c => ("Albanian".to_string(), "al".to_string(), false),
        0x042f => ("Macedonian".to_string(), "mk".to_string(), false),

        // Default case
        _ => {
            // Try to determine if it's English based on primary language
            let primary_lang = lang_id & 0x3FF;
            let is_english = primary_lang == 0x09; // LANG_ENGLISH
            let name = format!("Unknown Language (0x{:04X})", lang_id);
            let code = format!("{:02x}", (lang_id & 0xFF) as u8);
            (name, code, is_english)
        }
    }
}

pub fn validate_country_codes(codes: &[&str]) -> Result<Vec<String>, String> {
    let all_layouts = get_all_keyboard_layouts();
    let mut valid_codes = Vec::new();
    let mut invalid_codes = Vec::new();

    for code in codes {
        if all_layouts.iter().any(|l| l.short_code == *code) {
            valid_codes.push(code.to_string());
        } else {
            invalid_codes.push(code.to_string());
        }
    }

    if !invalid_codes.is_empty() {
        return Err(format!(
            "Unknown country codes: {}. Use 'ccaps -status' to see available codes.",
            invalid_codes.join(", ")
        ));
    }

    if valid_codes.is_empty() {
        return Err("No valid country codes provided.".to_string());
    }

    Ok(valid_codes)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_request_goes_to_foreground_window_without_focus() {
        let foreground = 0x1234 as HWND;
        assert_eq!(
            layout_request_target(foreground, ptr::null_mut()),
            Some(foreground)
        );
    }

    #[test]
    fn test_layout_request_goes_to_focused_control() {
        // E.g. the file name field of the Explorer "Save As" dialog
        let dialog = 0x1234 as HWND;
        let edit = 0x5678 as HWND;
        assert_eq!(layout_request_target(dialog, edit), Some(edit));
    }

    #[test]
    fn test_layout_request_never_broadcasts() {
        assert_eq!(layout_request_target(HWND_BROADCAST, ptr::null_mut()), None);
        let foreground = 0x1234 as HWND;
        assert_eq!(
            layout_request_target(foreground, HWND_BROADCAST),
            Some(foreground)
        );
    }

    #[test]
    fn test_exe_file_name_from_full_path() {
        assert_eq!(
            exe_file_name(r"C:\Program Files\Microsoft VS Code\Code.exe"),
            "Code.exe"
        );
        assert_eq!(exe_file_name("notepad.exe"), "notepad.exe");
    }

    #[test]
    fn test_unknown_thread_has_no_layout() {
        // GetKeyboardLayout(0) would return CCaps's own layout, not "unknown"
        assert_eq!(thread_layout(0), 0);
    }

    #[test]
    fn test_readable_layout_prefers_focus_thread() {
        // Notepad / UWP: the top-level window's thread may hold a stale layout
        assert_eq!(
            first_readable_layout(&[&|| 0x0419_0419, &|| 0x0407_0407, &|| 0x0409_0409]),
            Some(0x0419_0419)
        );
    }

    #[test]
    fn test_readable_layout_falls_back_to_ime_thread_for_consoles() {
        // conhost: focus and window threads read as 0, the IME window's thread does not
        assert_eq!(
            first_readable_layout(&[&|| 0, &|| 0, &|| 0x0419_0419]),
            Some(0x0419_0419)
        );
    }

    #[test]
    fn test_unreadable_layout_is_none() {
        assert_eq!(first_readable_layout(&[&|| 0, &|| 0, &|| 0]), None);
    }

    #[test]
    fn test_readable_layout_stops_at_first_hit() {
        let later_source_called = std::cell::Cell::new(false);
        let later = || {
            later_source_called.set(true);
            0x0409_0409
        };
        assert_eq!(
            first_readable_layout(&[&|| 0x0419_0419, &later]),
            Some(0x0419_0419)
        );
        assert!(!later_source_called.get());
    }

    #[test]
    fn test_layout_request_skipped_without_foreground_window() {
        assert_eq!(layout_request_target(ptr::null_mut(), 0x5678 as HWND), None);
    }
}
