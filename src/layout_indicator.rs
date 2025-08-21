use std::ptr;
use std::mem;
use winapi::um::winuser::*;
use winapi::shared::minwindef::HKL;

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
    match lang_id {
        0x0409 | 0x0809 | 0x0c09 | 0x1009 | 0x1409 | 
        0x1809 | 0x1c09 | 0x2009 | 0x2409 | 0x2809 | 
        0x2c09 | 0x3009 | 0x3409 => true,
        _ => false,
    }
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
        if hwnd.is_null() {
            return false;
        }
        
        // Get window thread ID
        let thread_id = GetWindowThreadProcessId(hwnd, ptr::null_mut());
        
        // Get current layout
        let current_layout = GetKeyboardLayout(thread_id);
        
        is_english_layout_hkl(current_layout)
    }
}

// Public function to update Scroll Lock indicator with specific layout
pub unsafe fn update_layout_indicator_with_layout(layout: HKL) {
    unsafe {
        let is_english = is_english_layout_hkl(layout);
        
        // English layout: Scroll Lock OFF
        // Non-English layout: Scroll Lock ON
        set_scroll_lock_state(!is_english);
    }
}

// Public function to update Scroll Lock indicator based on current layout
pub unsafe fn update_layout_indicator() {
    unsafe {
        let is_english = is_english_layout();
        
        // English layout: Scroll Lock OFF
        // Non-English layout: Scroll Lock ON
        set_scroll_lock_state(!is_english);
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