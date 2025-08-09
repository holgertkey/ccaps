use std::env;
use std::ptr;
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use winapi::um::winuser::*;
use winapi::um::winreg::*;
use winapi::um::handleapi::CloseHandle;
use winapi::um::synchapi::CreateMutexW;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::winnt::{KEY_SET_VALUE, REG_SZ, HANDLE};
use winapi::shared::minwindef::*;
use winapi::shared::winerror::*;

const MUTEX_NAME: &str = "Global\\CCapsLayoutSwitcherMutex";
const REGISTRY_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_NAME: &str = "CCaps Layout Switcher";

pub enum CliCommand {
    Start,
    Stop,
    Exit,
    Run, // Default run mode (no parameters)
    Background, // Internal command for background process
    Help,
    Unknown(String),
}

pub fn parse_args() -> CliCommand {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        return CliCommand::Run;
    }
    
    match args[1].as_str() {
        "-start" => CliCommand::Start,
        "-stop" => CliCommand::Stop,
        "-exit" => CliCommand::Exit,
        "--background" => CliCommand::Background,
        "-help" | "--help" | "/?" => CliCommand::Help,
        _ => CliCommand::Unknown(args[1].clone()),
    }
}

pub fn execute_command(command: CliCommand) -> i32 {
    match command {
        CliCommand::Start => handle_start(),
        CliCommand::Stop => handle_stop(),
        CliCommand::Exit => handle_exit(),
        CliCommand::Background | CliCommand::Run => 0, // Continue normal execution
        CliCommand::Help => {
            show_help();
            0
        },
        CliCommand::Unknown(cmd) => {
            eprintln!("Unknown command: {}", cmd);
            show_help();
            1
        }
    }
}

fn handle_start() -> i32 {
    println!("Starting CCaps Layout Switcher...");
    
    // Check if already running
    if is_already_running() {
        println!("The program is already running in the background.");
        return 1;
    }
    
    // Add to startup
    if let Err(e) = add_to_startup() {
        eprintln!("Warning: Could not add to startup: {}", e);
    } else {
        println!("Added to system startup.");
    }
    
    // Start in background (completely detached process)
    if let Err(e) = start_background_process() {
        eprintln!("Failed to start background process: {}", e);
        return 1;
    }
    
    println!("CCaps Layout Switcher started in background.");
    0
}

fn handle_stop() -> i32 {
    println!("Stopping CCaps Layout Switcher...");
    
    // Remove from startup
    if let Err(e) = remove_from_startup() {
        eprintln!("Warning: Could not remove from startup: {}", e);
    } else {
        println!("Removed from system startup.");
    }
    
    // Stop running process
    if stop_background_process() {
        println!("Background process stopped.");
    } else {
        println!("No background process was running.");
    }
    
    0
}

fn handle_exit() -> i32 {
    println!("Exiting CCaps Layout Switcher...");
    
    if stop_background_process() {
        println!("Background process stopped.");
    } else {
        println!("No background process was running.");
    }
    
    0
}

fn show_help() {
    println!("CCaps Layout Switcher v0.3.0");
    println!("Keyboard layout switcher using Caps Lock key");
    println!();
    println!("Usage:");
    println!("  ccaps          - Run in foreground mode");
    println!("  ccaps -start   - Start in background and add to system startup");
    println!("  ccaps -stop    - Stop background process and remove from startup");
    println!("  ccaps -exit    - Stop background process only");
    println!("  ccaps -help    - Show this help");
    println!();
    println!("Key bindings:");
    println!("  Caps Lock              - Switch keyboard layout");
    println!("  Alt + Caps Lock        - Toggle Caps Lock");
    println!("  Scroll Lock indicator  - Shows current layout (OFF=English, ON=Non-English)");
}

fn is_already_running() -> bool {
    unsafe {
        let mutex_name = format!("{}\0", MUTEX_NAME);
        let mutex_name_wide: Vec<u16> = OsString::from(mutex_name)
            .encode_wide()
            .collect();
        
        let mutex = CreateMutexW(
            ptr::null_mut(),
            FALSE,
            mutex_name_wide.as_ptr(),
        );
        
        if mutex.is_null() {
            return false;
        }
        
        let error = GetLastError();
        CloseHandle(mutex);
        
        error == ERROR_ALREADY_EXISTS
    }
}

fn add_to_startup() -> Result<(), String> {
    unsafe {
        let mut key: HKEY = ptr::null_mut();
        let key_name = format!("{}\0", REGISTRY_KEY);
        let key_name_wide: Vec<u16> = OsString::from(key_name)
            .encode_wide()
            .collect();
        
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key_name_wide.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        );
        
        if result != ERROR_SUCCESS as i32 {
            return Err("Failed to open registry key".to_string());
        }
        
        // Get current executable path
        let exe_path = env::current_exe()
            .map_err(|_| "Failed to get executable path")?;
        let exe_path_str = format!("\"{}\" --background\0", exe_path.display());
        let exe_path_wide: Vec<u16> = OsString::from(exe_path_str)
            .encode_wide()
            .collect();
        
        let app_name_wide: Vec<u16> = OsString::from(format!("{}\0", APP_NAME))
            .encode_wide()
            .collect();
        
        let result = RegSetValueExW(
            key,
            app_name_wide.as_ptr(),
            0,
            REG_SZ,
            exe_path_wide.as_ptr() as *const u8,
            (exe_path_wide.len() * 2) as u32,
        );
        
        RegCloseKey(key);
        
        if result == ERROR_SUCCESS as i32 {
            Ok(())
        } else {
            Err("Failed to set registry value".to_string())
        }
    }
}

fn remove_from_startup() -> Result<(), String> {
    unsafe {
        let mut key: HKEY = ptr::null_mut();
        let key_name = format!("{}\0", REGISTRY_KEY);
        let key_name_wide: Vec<u16> = OsString::from(key_name)
            .encode_wide()
            .collect();
        
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key_name_wide.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        );
        
        if result != ERROR_SUCCESS as i32 {
            return Err("Failed to open registry key".to_string());
        }
        
        let app_name_wide: Vec<u16> = OsString::from(format!("{}\0", APP_NAME))
            .encode_wide()
            .collect();
        
        let result = RegDeleteValueW(key, app_name_wide.as_ptr());
        RegCloseKey(key);
        
        if result == ERROR_SUCCESS as i32 || result == ERROR_FILE_NOT_FOUND as i32 {
            Ok(())
        } else {
            Err("Failed to remove registry value".to_string())
        }
    }
}

// Упрощенная функция создания отвязанного процесса через std::process
fn start_background_process() -> Result<(), String> {
    use std::process::Command;
    
    let exe_path = env::current_exe()
        .map_err(|_| "Failed to get executable path")?;
    
    // Простой запуск без дополнительных флагов
    match Command::new(&exe_path)
        .arg("--background")
        .spawn()
    {
        Ok(child) => {
            // Получаем PID дочернего процесса
            let _pid = child.id();
            
            // Отвязываем дочерний процесс - не ждем его завершения
            std::mem::forget(child);
            Ok(())
        },
        Err(e) => Err(format!("Failed to start process: {}", e)),
    }
}

// Альтернативная функция для создания демон-процесса через службу
// fn start_as_windows_service() -> Result<(), String> {
//     // Для более продвинутой реализации можно использовать Windows Service API
//     // Но для простого случая достаточно CREATE_NEW_CONSOLE | DETACHED_PROCESS
//     start_detached_process()
// }

fn stop_background_process() -> bool {
    // Send quit message to running instance
    unsafe {
        let window = FindWindowA(ptr::null(), b"CCaps Layout Switcher\0".as_ptr() as *const i8);
        if !window.is_null() {
            PostMessageA(window, WM_QUIT, 0, 0);
            return true;
        }
    }
    false
}

pub fn create_mutex() -> HANDLE {
    unsafe {
        let mutex_name = format!("{}\0", MUTEX_NAME);
        let mutex_name_wide: Vec<u16> = OsString::from(mutex_name)
            .encode_wide()
            .collect();
        
        CreateMutexW(
            ptr::null_mut(),
            TRUE,
            mutex_name_wide.as_ptr(),
        )
    }
}

pub fn should_run_in_background() -> bool {
    let args: Vec<String> = env::args().collect();
    args.len() > 1 && (args[1] == "--background")
}