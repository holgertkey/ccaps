mod keyboard_hook;
mod layout_indicator;
mod cli;

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
use keyboard_hook::{install_hook, uninstall_hook};
use cli::{parse_args, execute_command, CliCommand, create_mutex, should_run_in_background};

// Global atomic pointer to store mutex handle
static MUTEX_HANDLE: AtomicPtr<winapi::ctypes::c_void> = AtomicPtr::new(ptr::null_mut());

fn main() {
    // Parse command line arguments
    let command = parse_args();
    
    // Handle CLI commands that don't require running the main loop
    match command {
        CliCommand::Start | CliCommand::Stop | CliCommand::Exit | CliCommand::Help | CliCommand::Unknown(_) => {
            let exit_code = execute_command(command);
            std::process::exit(exit_code);
        }
        CliCommand::Run | CliCommand::Background => {
            // Continue with normal execution
        }
    }
    
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
            println!("The program is already running in the background.");
            CloseHandle(mutex);
            return;
        }
    }
    
    let is_background = should_run_in_background();
    
    if is_background {
        // Отвязываемся от родительского процесса в фоновом режиме
        unsafe {
            detach_from_console();
        }
    } else {
        // Show startup message in foreground mode
        println!("Caps Lock Layout Switcher started!");
        println!("Caps Lock - switch keyboard layout");
        println!("Alt + Caps Lock - toggle Caps Lock");
        println!("Scroll Lock indicator shows current layout:");
        println!("  OFF = English layout");
        println!("  ON  = Non-English layout");
        println!("Press Ctrl+C to exit");
    }
    
    unsafe {
        // Show current layout info only in foreground mode
        let (layout_name, is_english) = layout_indicator::get_current_layout_info();
        if !is_background {
            println!("Current layout: {} (English: {})", layout_name, is_english);
            println!("Setting Scroll Lock to: {}", if is_english { "OFF" } else { "ON" });
        }
        
        // Set initial Scroll Lock state based on current layout
        layout_indicator::update_layout_indicator();
        
        // Install the hook
        match install_hook() {
            Ok(()) => {
                if !is_background {
                    println!("Hook installed successfully");
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
        
        // Ctrl+C handler for proper shutdown (только для foreground режима)
        if !is_background {
            ctrlc::set_handler(move || {
                println!("\nShutting down...");
                cleanup_and_exit();
                std::process::exit(0);
            }).expect("Error setting Ctrl+C handler");
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

// Упрощенная функция для отвязки от консоли
unsafe fn detach_from_console() {
    // Скрываем консольное окно
    let console_window = GetConsoleWindow();
    if !console_window.is_null() {
        ShowWindow(console_window, SW_HIDE);
    }
    
    // Отвязываемся от консоли родительского процесса
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

// Улучшенная функция создания скрытого окна
unsafe fn create_message_window() {
    use winapi::um::libloaderapi::GetModuleHandleW;
    
    let class_name = "CCapsMessageWindow\0";
    let class_name_wide: Vec<u16> = OsString::from(class_name).encode_wide().collect();
    
    let window_name = "CCaps Layout Switcher\0";
    let window_name_wide: Vec<u16> = OsString::from(window_name).encode_wide().collect();
    
    // Кастомная процедура окна для обработки системных сообщений
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: UINT,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_QUERYENDSESSION | WM_ENDSESSION => {
                // Система завершается - выходим корректно
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
        HWND_MESSAGE, // Message-only window (не отображается в UI)
        ptr::null_mut(),
        GetModuleHandleW(ptr::null()),
        ptr::null_mut(),
    );
    
    if hwnd.is_null() {
        // Fallback: попробуем создать обычное скрытое окно
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