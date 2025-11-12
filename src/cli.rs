use std::env;
use std::ptr;
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use winapi::um::winuser::*;
use winapi::um::winreg::*;
use winapi::um::handleapi::CloseHandle;
use winapi::um::synchapi::CreateMutexW;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::winnt::{KEY_SET_VALUE, KEY_QUERY_VALUE, REG_SZ, HANDLE};
use winapi::shared::minwindef::*;
use winapi::shared::winerror::*;
use crate::layout_manager;
use crate::config;

const MUTEX_NAME: &str = "Global\\CCapsLayoutSwitcherMutex";
const REGISTRY_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_NAME: &str = "CCaps Layout Switcher";

pub enum CliCommand {
    Start(Vec<String>), // Modified to include country codes
    Stop,
    Exit,
    Status,
    Run(Vec<String>), // Modified to include country codes
    Menu, // Interactive menu (no parameters)
    Background(Vec<String>), // Internal command for background process with country codes
    Help,
    Version,
    Unknown(String),
}

pub fn parse_args() -> CliCommand {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        // No arguments provided - show interactive menu
        return CliCommand::Menu;
    }
    
    match args[1].as_str() {
        "-start" => {
            // Parse country codes after -start
            let country_codes: Vec<String> = args[2..].iter()
                .filter(|arg| arg.starts_with('-') && arg.len() > 1)
                .map(|arg| arg[1..].to_string())
                .collect();
            CliCommand::Start(country_codes)
        },
        "-stop" => CliCommand::Stop,
        "-exit" => CliCommand::Exit,
        "-status" => CliCommand::Status,
        "-run" => {
            // Parse country codes after -run
            let country_codes: Vec<String> = args[2..].iter()
                .filter(|arg| arg.starts_with('-') && arg.len() > 1)
                .map(|arg| arg[1..].to_string())
                .collect();
            CliCommand::Run(country_codes)
        },
        "--background" => {
            // Parse country codes after --background
            let country_codes: Vec<String> = args[2..].iter()
                .filter(|arg| arg.starts_with('-') && arg.len() > 1)
                .map(|arg| arg[1..].to_string())
                .collect();
            CliCommand::Background(country_codes)
        },
        "-help" | "--help" | "-h" | "/?" => CliCommand::Help,
        "-v" | "--version" => CliCommand::Version,
        _ => CliCommand::Unknown(args[1].clone()),
    }
}

pub fn execute_command(command: CliCommand) -> (i32, Vec<String>) {
    match command {
        CliCommand::Start(country_codes) => (handle_start(&country_codes), vec![]),
        CliCommand::Stop => (handle_stop(), vec![]),
        CliCommand::Exit => (handle_exit(), vec![]),
        CliCommand::Status => (handle_status(), vec![]),
        CliCommand::Background(country_codes) => (handle_background(&country_codes), country_codes),
        CliCommand::Run(country_codes) => (0, country_codes), // Continue normal execution
        CliCommand::Menu => (0, vec![]), // This should not be called directly
        CliCommand::Help => {
            show_help();
            (0, vec![])
        },
        CliCommand::Version => {
            show_version();
            (0, vec![])
        },
        CliCommand::Unknown(cmd) => {
            eprintln!("Unknown command: {}", cmd);
            // show_help();
            (1, vec![])
        }
    }
}

fn handle_background(country_codes: &[String]) -> i32 {
    // Load configuration from file if no country codes provided
    let final_country_codes = if country_codes.is_empty() {
        let config = config::load_config();
        config.country_codes
    } else {
        country_codes.to_vec()
    };
    
    // When running in the background, we only check autoload
    // but we don't install it again
    if !is_in_startup() {
        // If for some reason the autoload is missing, we add it
        if let Err(e) = add_to_startup(&final_country_codes) {
            eprintln!("Warning: Could not ensure startup entry: {}", e);
        }
    }
    
    // Continue normal execution (do not terminate the program)
    0
}

fn handle_status() -> i32 {
    println!("CCaps Layout Switcher Status:");
    println!("╞══════════════════════════════════════════════════════════════╡");
    
    // Check if running in background
    let is_running = is_already_running();
    println!("Background process: {}", if is_running { "RUNNING ✓" } else { "NOT RUNNING ✗" });
    
    // Check startup entry
    let in_startup = is_in_startup();
    println!("Auto-startup:       {}", if in_startup { "ENABLED ✓" } else { "DISABLED ✗" });
    
    // Show startup path if enabled
    if in_startup {
        if let Ok(startup_path) = get_startup_path() {
            println!("Startup command:    {}", startup_path);
        }
    }
    
    // Show configuration info
    let config = config::load_config();
    let (config_exists, config_path) = config::get_config_info();
    println!("Configuration file: {}", if config_exists { "EXISTS ✓" } else { "NOT FOUND ✗" });
    if let Some(path) = config_path {
        println!("Config path:        {}", path);
    }
    if !config.country_codes.is_empty() {
        println!("Saved country codes: {}", config.country_codes.join(", "));
    } else {
        println!("Saved country codes: [All layouts]");
    }
    
    println!();
    
    // Show available layouts
    let layouts = layout_manager::get_all_keyboard_layouts();
    println!("Available keyboard layouts:");
    println!("┌─────┬──────────────────────────────────────┬─────────────────┐");
    println!("│ Code│ Language                             │ Status          │");
    println!("├─────┼──────────────────────────────────────┼─────────────────┤");
    
    let current_layout = layout_manager::get_current_layout();
    
    for layout in &layouts {
        let status = if let Some(ref current) = current_layout {
            if current.hkl == layout.hkl {
                "CURRENT ✓"
            } else {
                "Available"
            }
        } else {
            "Available"
        };
        
        println!("│ -{:<2} │ {:<36} │ {:<15} │", 
                layout.short_code, layout.name, status);
    }
    println!("└─────┴──────────────────────────────────────┴─────────────────┘");
    
    println!();
    println!("Usage examples:");
    println!("  ccaps -run            # Run in foreground mode (cycle through all layouts)");
    println!("  ccaps -run -de        # Switch between English and German");
    println!("  ccaps -run -de -fr    # Switch between German and French");
    println!("  ccaps -start          # Start with all layouts and add to auto-startup");
    println!("  ccaps -start -de      # Start with English/German and add to auto-startup");
    println!();
    
    // Show recommendations
    match (is_running, in_startup) {
        (true, true) => println!("Status: All systems operational ✓"),
        (true, false) => {
            println!("Status: Running but not in auto-startup");
            println!("Recommendation: Run 'ccaps -start' to enable auto-startup");
        },
        (false, true) => {
            println!("Status: Auto-startup enabled but not currently running");
            println!("Recommendation: Run 'ccaps -start' to start background process");
        },
        (false, false) => {
            println!("Status: Not running and auto-startup disabled");
            println!("Recommendation: Run 'ccaps -start' to start and enable auto-startup");
        }
    }
    
    0
}

fn handle_start(country_codes: &[String]) -> i32 {
    println!("Starting CCaps Layout Switcher...");
    
    // Check if already running
    if is_already_running() {
        println!("The program is already running in the background.");
        return 1;
    }
    
    // Validate country codes if provided
    if !country_codes.is_empty() {
        if let Err(error) = layout_manager::validate_country_codes(
            &country_codes.iter().map(|s| s.as_str()).collect::<Vec<_>>()
        ) {
            eprintln!("Error: {}", error);
            return 1;
        }
        println!("Using country codes: {}", country_codes.join(", "));
    } else {
        println!("Using all available layouts");
    }
    
    // Save configuration
    let config = config::Config::with_country_codes(country_codes.to_vec());
    if let Err(e) = config::save_config(&config) {
        eprintln!("Warning: Could not save configuration: {}", e);
    } else {
        println!("Configuration saved.");
    }
    
    // Add to startup with country codes
    if let Err(e) = add_to_startup(country_codes) {
        eprintln!("Warning: Could not add to startup: {}", e);
    } else {
        println!("Added to system startup.");
    }
    
    // Start in background (completely detached process)
    if let Err(e) = start_background_process(country_codes) {
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
    
    // Delete configuration file
    if let Err(e) = config::delete_config() {
        eprintln!("Warning: Could not delete configuration: {}", e);
    } else {
        println!("Configuration deleted.");
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

fn show_version() {
    println!("CCaps Layout Switcher v{}", env!("CARGO_PKG_VERSION"));
}

fn show_help() {
    println!("CCaps Layout Switcher v{}", env!("CARGO_PKG_VERSION"));
    println!("Keyboard layout switcher using Caps Lock key");
    println!();
    println!("Usage:");
    println!("  ccaps              - Show interactive menu");
    println!("  ccaps -run         - Run in foreground mode (cycle through all layouts)");
    println!("  ccaps -run -de     - Run with English ↔ German switching");
    println!("  ccaps -run -de -fr - Run with German ↔ French switching");
    println!("  ccaps -start       - Start in background with all layouts and add to auto-startup");
    println!("  ccaps -start -de   - Start in background with German/English and add to auto-startup");
    println!("  ccaps -stop        - Stop background process and remove from startup");
    println!("  ccaps -exit        - Stop background process only");
    println!("  ccaps -status      - Show current status and available language codes");
    println!("  ccaps -help        - Show this help");
    println!("  ccaps -v           - Show version information");
    println!();
    println!("Key bindings:");
    println!("  Caps Lock              - Switch keyboard layout");
    println!("  Alt + Caps Lock        - Toggle Caps Lock");
    println!("  Scroll Lock indicator  - Shows current layout (OFF=English, ON=Non-English)");
    println!();
    println!("Configuration:");
    println!("  Settings are automatically saved when using -start with country codes");
    println!("  Configuration file: ccaps-config.json (in program directory)");
    println!("  Use 'ccaps -status' to see all available language codes");
    println!();
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

fn is_in_startup() -> bool {
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
            KEY_QUERY_VALUE,
            &mut key,
        );
        
        if result != ERROR_SUCCESS as i32 {
            return false;
        }
        
        let app_name_wide: Vec<u16> = OsString::from(format!("{}\0", APP_NAME))
            .encode_wide()
            .collect();
        
        let mut value_type: DWORD = 0;
        let mut data_size: DWORD = 0;
        
        let result = RegQueryValueExW(
            key,
            app_name_wide.as_ptr(),
            ptr::null_mut(),
            &mut value_type,
            ptr::null_mut(),
            &mut data_size,
        );
        
        RegCloseKey(key);
        
        result == ERROR_SUCCESS as i32 && value_type == REG_SZ && data_size > 0
    }
}

fn get_startup_path() -> Result<String, String> {
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
            KEY_QUERY_VALUE,
            &mut key,
        );
        
        if result != ERROR_SUCCESS as i32 {
            return Err("Failed to open registry key".to_string());
        }
        
        let app_name_wide: Vec<u16> = OsString::from(format!("{}\0", APP_NAME))
            .encode_wide()
            .collect();
        
        let mut value_type: DWORD = 0;
        let mut data_size: DWORD = 0;
        
        // First call to get size
        let result = RegQueryValueExW(
            key,
            app_name_wide.as_ptr(),
            ptr::null_mut(),
            &mut value_type,
            ptr::null_mut(),
            &mut data_size,
        );
        
        if result != ERROR_SUCCESS as i32 {
            RegCloseKey(key);
            return Err("Failed to query registry value size".to_string());
        }
        
        // Allocate buffer and get actual value
        let mut buffer: Vec<u16> = vec![0; (data_size / 2) as usize];
        let result = RegQueryValueExW(
            key,
            app_name_wide.as_ptr(),
            ptr::null_mut(),
            &mut value_type,
            buffer.as_mut_ptr() as *mut u8,
            &mut data_size,
        );
        
        RegCloseKey(key);
        
        if result == ERROR_SUCCESS as i32 && value_type == REG_SZ {
            // Convert wide string to String, removing null terminator
            let end_pos = buffer.iter().position(|&x| x == 0).unwrap_or(buffer.len());
            let path = String::from_utf16(&buffer[..end_pos])
                .map_err(|_| "Failed to convert path to string")?;
            Ok(path)
        } else {
            Err("Failed to read registry value".to_string())
        }
    }
}

fn add_to_startup(country_codes: &[String]) -> Result<(), String> {
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
        
        let mut exe_path_str = format!("\"{}\" --background", exe_path.display());
        
        // Add country codes to the command line
        for code in country_codes {
            exe_path_str.push_str(&format!(" -{}", code));
        }
        exe_path_str.push('\0');
        
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

fn start_background_process(country_codes: &[String]) -> Result<(), String> {
    use std::process::Command;
    
    let exe_path = env::current_exe()
        .map_err(|_| "Failed to get executable path")?;

    let mut command = Command::new(&exe_path);
    command.arg("--background");
    
    // Add country codes to the background process
    for code in country_codes {
        command.arg(&format!("-{}", code));
    }

    // Simple launch without additional flags
    match command.spawn() {
        Ok(child) => {
            // Get the PID of the child process
            let _pid = child.id();
            
            // Unbind the child process - don't wait for it to complete
            std::mem::forget(child);
            Ok(())
        },
        Err(e) => Err(format!("Failed to start process: {}", e)),
    }
}

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