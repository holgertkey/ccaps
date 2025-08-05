mod keyboard_hook;

use std::ptr;
use std::mem;
use winapi::um::winuser::*;
use keyboard_hook::{install_hook, uninstall_hook};

fn main() {
    println!("Caps Lock Layout Switcher started!");
    println!("Caps Lock - switch keyboard layout");
    println!("Alt + Caps Lock - toggle Caps Lock");
    println!("Press Ctrl+C to exit");
    
    unsafe {
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








