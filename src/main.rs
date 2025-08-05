mod keyboard_hook;
mod layout_indicator;

use std::ptr;
use std::mem;
use winapi::um::winuser::*;
use keyboard_hook::{install_hook, uninstall_hook};

fn main() {
    println!("Caps Lock Layout Switcher started!");
    println!("Caps Lock - switch keyboard layout");
    println!("Alt + Caps Lock - toggle Caps Lock");
    println!("Scroll Lock indicator shows current layout:");
    println!("  OFF = English layout");
    println!("  ON  = Non-English layout");
    println!("Press Ctrl+C to exit");
    
    unsafe {
        // Show current layout info
        let (layout_name, is_english) = layout_indicator::get_current_layout_info();
        println!("Current layout: {} (English: {})", layout_name, is_english);
        
        // Set initial Scroll Lock state based on current layout
        layout_indicator::update_layout_indicator();
        
        // Install the hook
        match install_hook() {
            Ok(()) => println!("Hook installed successfully"),
            Err(e) => {
                eprintln!("Hook installation error: {}", e);
                return;
            }
        }
        
        // Ctrl+C handler for proper shutdown
        ctrlc::set_handler(move || {
            println!("\nShutting down...");
            uninstall_hook();
            std::process::exit(0);
        }).expect("Error setting Ctrl+C handler");
        
        // Main message processing loop
        let mut msg: MSG = mem::zeroed();
        loop {
            let result = GetMessageW(&mut msg, ptr::null_mut(), 0, 0);
            if result == 0 || result == -1 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        // Remove hook on exit
        uninstall_hook();
    }
}






