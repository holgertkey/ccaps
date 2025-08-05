use std::ptr;
use std::mem;
use winapi::um::winuser::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;

// Global variable to store the hook
static mut HOOK: HHOOK = ptr::null_mut();

// Structure for passing data to the hook
struct HookData {
    alt_pressed: bool,
}

static mut HOOK_DATA: HookData = HookData { alt_pressed: false };

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
                if w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize {
                    HOOK_DATA.alt_pressed = true;
                } else if w_param == WM_KEYUP as usize || w_param == WM_SYSKEYUP as usize {
                    HOOK_DATA.alt_pressed = false;
                }
            }
            
            // Handle Caps Lock press
            if vk_code == VK_CAPITAL as u32 && (w_param == WM_KEYDOWN as usize) {
                if HOOK_DATA.alt_pressed {
                    // Alt + Caps Lock: toggle Caps Lock functionality
                    toggle_caps_lock();
                } else {
                    // Caps Lock only: switch keyboard layout
                    switch_keyboard_layout();
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
    unsafe {
        // Get active window
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return;
        }
        
        // Get window thread ID
        let thread_id = GetWindowThreadProcessId(hwnd, ptr::null_mut());
        
        // Get current layout
        let current_layout = GetKeyboardLayout(thread_id);
        
        // Get list of all available layouts
        let mut layouts: [HKL; 10] = mem::zeroed();
        let layout_count = GetKeyboardLayoutList(10, layouts.as_mut_ptr());
        
        if layout_count > 1 {
            // Find current layout in the list and switch to the next one
            let mut next_layout = layouts[0];
            for i in 0..layout_count as usize {
                if layouts[i] == current_layout {
                    next_layout = layouts[(i + 1) % (layout_count as usize)];
                    break;
                }
            }
            
            // Activate new layout
            ActivateKeyboardLayout(next_layout, 0);
            
            // Send layout change message to all windows
            PostMessageW(
                HWND_BROADCAST,
                WM_INPUTLANGCHANGEREQUEST,
                0,
                next_layout as LPARAM,
            );
        }
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