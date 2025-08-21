use std::ptr;
use std::mem;
use std::sync::Mutex;
use winapi::um::winuser::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::shared::minwindef::*;
use winapi::shared::windef::HHOOK;
use crate::layout_indicator;
use crate::layout_manager::{self, LayoutInfo};

// Global variable to store the hook
static mut HOOK: HHOOK = ptr::null_mut();

// Structure for passing data to the hook
struct HookData {
    alt_pressed: bool,
    selected_layouts: Vec<LayoutInfo>,
    current_layout_index: usize,
}

// Use a Mutex to protect the hook data
static HOOK_DATA: Mutex<HookData> = Mutex::new(HookData {
    alt_pressed: false,
    selected_layouts: Vec::new(),
    current_layout_index: 0,
});

// Initialize layout switching with specific country codes
pub fn initialize_layout_switching(country_codes: &[String]) {
    let mut hook_data = HOOK_DATA.lock().unwrap();
    
    if country_codes.is_empty() {
        // Use all available layouts
        hook_data.selected_layouts = layout_manager::get_all_keyboard_layouts();
    } else {
        // Find layouts by country codes
        let codes: Vec<&str> = country_codes.iter().map(|s| s.as_str()).collect();
        hook_data.selected_layouts = layout_manager::find_layouts_by_codes(&codes);
        
        // If no layouts found by codes or only one layout found, 
        // try to add English layout for better switching experience
        if hook_data.selected_layouts.len() <= 1 {
            if let Some(english_layout) = layout_manager::get_english_layout() {
                // Add English layout if not already present
                let has_english = hook_data.selected_layouts.iter()
                    .any(|l| l.hkl == english_layout.hkl);
                
                if !has_english {
                    hook_data.selected_layouts.insert(0, english_layout);
                }
            }
        }
    }
    
    // Find current layout index
    if let Some(current) = layout_manager::get_current_layout() {
        hook_data.current_layout_index = hook_data.selected_layouts
            .iter()
            .position(|l| l.hkl == current.hkl)
            .unwrap_or(0);
    }
    
    println!("Initialized with {} layout(s):", hook_data.selected_layouts.len());
    for (i, layout) in hook_data.selected_layouts.iter().enumerate() {
        let marker = if i == hook_data.current_layout_index { " [CURRENT]" } else { "" };
        println!("  {} - {}{}", layout.short_code, layout.name, marker);
    }
}

// Callback function for handling key presses
unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    unsafe {
        if n_code >= 0 {
            let kb_struct = *(l_param as *const KBDLLHOOKSTRUCT);
            let vk_code = kb_struct.vkCode;
            
            // Track Alt key state
            if vk_code == VK_MENU as u32 {
                if let Ok(mut hook_data) = HOOK_DATA.lock() {
                    if w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize {
                        hook_data.alt_pressed = true;
                    } else if w_param == WM_KEYUP as usize || w_param == WM_SYSKEYUP as usize {
                        hook_data.alt_pressed = false;
                    }
                }
            }
            
            // Handle Caps Lock press
            if vk_code == VK_CAPITAL as u32 && (w_param == WM_KEYDOWN as usize) {
                if let Ok(hook_data) = HOOK_DATA.lock() {
                    if hook_data.alt_pressed {
                        // Alt + Caps Lock: toggle Caps Lock functionality
                        drop(hook_data); // Release lock before calling toggle_caps_lock
                        toggle_caps_lock();
                    } else {
                        // Caps Lock only: switch keyboard layout
                        drop(hook_data); // Release lock before calling switch_keyboard_layout
                        switch_keyboard_layout();
                    }
                }
                // Block default Caps Lock processing
                return 1;
            }
        }
        
        CallNextHookEx(HOOK, n_code, w_param, l_param)
    }
}

// Function to switch keyboard layout
unsafe fn switch_keyboard_layout() {
    if let Ok(mut hook_data) = HOOK_DATA.lock() {
        if hook_data.selected_layouts.is_empty() {
            return;
        }
        
        if hook_data.selected_layouts.len() == 1 {
            // Only one layout available, just activate it
            layout_manager::switch_to_layout(&hook_data.selected_layouts[0]);
            layout_indicator::update_layout_indicator_with_layout(hook_data.selected_layouts[0].get_hkl());
            return;
        }
        
        // Move to next layout
        hook_data.current_layout_index = (hook_data.current_layout_index + 1) % hook_data.selected_layouts.len();
        let next_layout = &hook_data.selected_layouts[hook_data.current_layout_index];
        
        // Switch to the new layout
        layout_manager::switch_to_layout(next_layout);
        
        // Update Scroll Lock indicator
        layout_indicator::update_layout_indicator_with_layout(next_layout.get_hkl());
    }
}

// Function to toggle Caps Lock state
unsafe fn toggle_caps_lock() {
    unsafe {
        // Get current Caps Lock state
        let _caps_state = GetKeyState(VK_CAPITAL);
        
        // Create array for simulating key press
        let mut inputs: [INPUT; 2] = mem::zeroed();
        
        // First INPUT - Caps Lock press
        inputs[0].type_ = INPUT_KEYBOARD;
        inputs[0].u.ki_mut().wVk = VK_CAPITAL as u16;
        inputs[0].u.ki_mut().dwFlags = 0;
        
        // Second INPUT - Caps Lock release
        inputs[1].type_ = INPUT_KEYBOARD;
        inputs[1].u.ki_mut().wVk = VK_CAPITAL as u16;
        inputs[1].u.ki_mut().dwFlags = KEYEVENTF_KEYUP;
        
        // Send press and release events
        SendInput(2, inputs.as_mut_ptr(), mem::size_of::<INPUT>() as i32);
    }
}

// Public function to install the hook
pub unsafe fn install_hook() -> Result<(), &'static str> {
    unsafe {
        let h_mod = GetModuleHandleW(ptr::null());
        if h_mod.is_null() {
            return Err("Failed to get module handle");
        }
        
        HOOK = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            h_mod,
            0,
        );
        
        if HOOK.is_null() {
            return Err("Failed to install hook");
        }
        
        Ok(())
    }
}

// Public function to uninstall the hook
pub unsafe fn uninstall_hook() {
    unsafe {
        if !HOOK.is_null() {
            UnhookWindowsHookEx(HOOK);
            HOOK = ptr::null_mut();
        }
    }
}

// Function to get current layout switching status (for debugging)
pub fn get_switching_status() -> (usize, Vec<String>) {
    if let Ok(hook_data) = HOOK_DATA.lock() {
        let layout_names: Vec<String> = hook_data.selected_layouts
            .iter()
            .map(|l| format!("{} ({})", l.name, l.short_code))
            .collect();
        (hook_data.current_layout_index, layout_names)
    } else {
        (0, vec!["Error: Could not access layout data".to_string()])
    }
}