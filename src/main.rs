mod keyboard_hook;
mod layout_indicator;
mod layout_manager;
mod cli;
mod interactive_menu;
mod config;

use std::ptr;
use std::mem;
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicPtr, Ordering};
use winapi::um::winuser::*;
use winapi::um::handleapi::CloseHandle;
use winapi::um::wincon::*;
use winapi::um::errhandlingapi::GetLastError;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use keyboard_hook::{install_hook, uninstall_hook, initialize_layout_switching};
use cli::{parse_args, execute_command, CliCommand, create_mutex, should_run_in_background};
use interactive_menu::show_interactive_menu;

// Global atomic pointer to store mutex handle
static MUTEX_HANDLE: AtomicPtr<winapi::ctypes::c_void> = AtomicPtr::new(ptr::null_mut());

fn main() {
    // Parse command line arguments
    let command = parse_args();
    
    // Handle CLI commands that don't require running the main loop
    match command {
        CliCommand::Start(_) | CliCommand::Stop | CliCommand::Exit | CliCommand::Status | CliCommand::Help | CliCommand::Unknown(_) => {
            let (exit_code, _) = execute_command(command);
            std::process::exit(exit_code);
        }
        CliCommand::Background(country_codes) => {
            // Execute background-specific logic and get country codes
            let (_, final_country_codes) = execute_command(CliCommand::Background(country_codes.clone()));
            
            // Load configuration from file if no country codes provided via command line
            let country_codes_to_use = if final_country_codes.is_empty() {
                let config = config::load_config();
                config.country_codes
            } else {
                final_country_codes
            };
            
            // Validate country codes if provided
            if !country_codes_to_use.is_empty() {
                if let Err(error) = layout_manager::validate_country_codes(
                    &country_codes_to_use.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                ) {
                    eprintln!("Error: {}", error);
                    std::process::exit(1);
                }
            }
            
            // Continue with normal execution after background setup
            run_main_loop(country_codes_to_use);
        }
        CliCommand::Menu => {
            // Show interactive menu when no parameters provided
            let (menu_result, country_codes) = show_interactive_menu();
            if menu_result != 0 {
                std::process::exit(menu_result);
            }
            // If menu_result is 0, continue with normal execution using country codes from menu
            run_main_loop(country_codes);
        }
        CliCommand::Run(country_codes) => {
            // Validate country codes if provided
            if !country_codes.is_empty() {
                if let Err(error) = layout_manager::validate_country_codes(
                    &country_codes.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                ) {
                    eprintln!("Error: {}", error);
                    std::process::exit(1);
                }
            }
            
            // Direct run mode - continue with normal execution
            run_main_loop(country_codes);
        }
    }
}

fn run_main_loop(country_codes: Vec<String>) {
    // Create mutex to prevent multiple instances
    let mutex = create_mutex();
    if mutex.is_null() {
        eprintln!("Failed to create mutex");
        return;
    }
    
    // Store mutex handle in atomic pointer
    MUTEX_HANDLE.store(mutex, Ordering::SeqCst);
    
    // Check if another instance is already running
    unsafe {
        if GetLastError() == winapi::shared::winerror::ERROR_ALREADY_EXISTS {
            if !should_run_in_background() {
                println!("The program is already running in the background.");
            }
            CloseHandle(mutex);
            return;
        }
    }
    
    let is_background = should_run_in_background();
    
    if is_background {
        // Detaching from the parent process in the background
        unsafe {
            detach_from_console();
        }
    } else {
        // Show startup message in foreground mode
        println!();
        println!("═══════════════════════════════════════════════════");
        println!("       Caps Lock Layout Switcher started!          ");
        println!("═══════════════════════════════════════════════════");
        println!("Alt + Caps Lock - toggle Caps Lock");
        println!("Scroll Lock indicator shows current layout:");
        println!("  OFF = English layout");
        println!("  ON  = Non-English layout");
        println!();
    }
    
    unsafe {
        // Initialize layout switching with country codes
        initialize_layout_switching(&country_codes);
        
        if !is_background {
            // Show current layout info only in foreground mode
            if let Some(current_layout) = layout_manager::get_current_layout() {
                println!("Current layout: {} ({})", current_layout.name, current_layout.short_code);
                println!("Setting Scroll Lock to: {}", if current_layout.is_english { "OFF" } else { "ON" });
            } else {
                println!("Could not detect current layout");
            }
            println!();
        }
        
        // Set initial Scroll Lock state based on current layout
        layout_indicator::update_layout_indicator();
        
        // Install the hook
        match install_hook() {
            Ok(()) => {
                if !is_background {
                    println!("Hook installed successfully");
                    println!("Layout switcher is now active!");
                    
                    // Show switching configuration
                    let (current_index, layout_names) = keyboard_hook::get_switching_status();
                    if !layout_names.is_empty() {
                        println!("Switching between {} layout(s):", layout_names.len());
                        for (i, name) in layout_names.iter().enumerate() {
                            let marker = if i == current_index { " [CURRENT]" } else { "" };
                            println!("  {}{}", name, marker);
                        }
                    }
                    
                    println!();
                    println!("Press Ctrl+C to exit");
                    println!();
                }
            },
            Err(e) => {
                if !is_background {
                    eprintln!("Hook installation error: {}", e);
                }
                cleanup_and_exit();
                return;
            }
        }
        
        // Ctrl+C handler for proper shutdown (only for foreground mode)
        if !is_background {
            // Try to set up Ctrl+C handler, but don't panic if it fails
            if let Err(e) = ctrlc::set_handler(move || {
                println!("\nShutting down...");
                cleanup_and_exit();
                std::process::exit(0);
            }) {
                println!("Warning: Could not set Ctrl+C handler: {}", e);
                println!("You may need to close the console window manually to exit.");
            }
        }
        
        // Create hidden window for message handling
        create_message_window();
        
        // Main message processing loop
        let mut msg: MSG = mem::zeroed();
        loop {
            let result = GetMessageW(&mut msg, ptr::null_mut(), 0, 0);
            if result == 0 || result == -1 {
                break;
            }
            
            // Handle quit message
            if msg.message == WM_QUIT {
                break;
            }
            
            // Handle other system messages
            if msg.message == WM_QUERYENDSESSION || msg.message == WM_ENDSESSION {
                // System shutdown - cleanup and exit gracefully
                break;
            }
            
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        // Cleanup
        cleanup_and_exit();
    }
}

// Simplified function to detach from console
unsafe fn detach_from_console() {
    // Hiding the console window
    let console_window = GetConsoleWindow();
    if !console_window.is_null() {
        ShowWindow(console_window, SW_HIDE);
    }
    
    // Unbind from the parent process console
    FreeConsole();
}

// Helper function to cleanup resources
fn cleanup_and_exit() {
    unsafe {
        uninstall_hook();
        let mutex = MUTEX_HANDLE.swap(ptr::null_mut(), Ordering::SeqCst);
        if !mutex.is_null() {
            CloseHandle(mutex);
        }
    }
}

// Improved hidden window creation function
unsafe fn create_message_window() {
    use winapi::um::libloaderapi::GetModuleHandleW;
    
    let class_name = "CCapsMessageWindow\0";
    let class_name_wide: Vec<u16> = OsString::from(class_name).encode_wide().collect();
    
    let window_name = "CCaps Layout Switcher\0";
    let window_name_wide: Vec<u16> = OsString::from(window_name).encode_wide().collect();
    
    // Custom window procedure for handling system messages
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: UINT,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_QUERYENDSESSION | WM_ENDSESSION => {
                // System shutdown - cleanup and exit gracefully
                PostQuitMessage(0);
                return 0;
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                return 0;
            }
            _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
    
    // Register window class
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: GetModuleHandleW(ptr::null()),
        hIcon: ptr::null_mut(),
        hCursor: ptr::null_mut(),
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name_wide.as_ptr(),
    };
    
    RegisterClassW(&wc);
    
    // Create hidden window
    let hwnd = CreateWindowExW(
        0,
        class_name_wide.as_ptr(),
        window_name_wide.as_ptr(),
        0, // No window style (completely hidden)
        0, 0, 0, 0, // Position and size (irrelevant for hidden window)
        HWND_MESSAGE, // Message-only window (not displayed in UI)
        ptr::null_mut(),
        GetModuleHandleW(ptr::null()),
        ptr::null_mut(),
    );
    
    if hwnd.is_null() {
        // Fallback: try to create a regular hidden window
        CreateWindowExW(
            0,
            class_name_wide.as_ptr(),
            window_name_wide.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            ptr::null_mut(),
            ptr::null_mut(),
            GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        );
    }
}