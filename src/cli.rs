use crate::config;
use crate::layout_manager;
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use winapi::shared::minwindef::*;
use winapi::shared::windef::HWND;
use winapi::shared::winerror::*;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::{
    CreateProcessW, GetExitCodeProcess, PROCESS_INFORMATION, STARTUPINFOW,
};
use winapi::um::synchapi::{CreateMutexW, WaitForSingleObject};
use winapi::um::winbase::{CREATE_NO_WINDOW, WAIT_OBJECT_0};
use winapi::um::winnt::{HANDLE, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ};
use winapi::um::winreg::*;
use winapi::um::winuser::*;

const MUTEX_NAME: &str = "Global\\CCapsLayoutSwitcherMutex";
const REGISTRY_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_NAME: &str = "CCaps Layout Switcher";

// Commands follow a systemctl-like model: -enable/-disable change the persistent state
// (config file and auto-startup), -start/-stop only affect the running process.
#[derive(Debug, PartialEq)]
pub enum CliCommand {
    Enable(Vec<String>), // Save config + add to auto-startup + start if not running
    Disable,             // Stop + remove from auto-startup + delete config
    Start(Vec<String>),  // Start the background process now; config and auto-startup untouched
    Stop,                // Stop the background process only
    Status,
    Run(Vec<String>),        // Run in foreground mode
    Menu,                    // Interactive menu (no parameters)
    Background(Vec<String>), // Internal command for background process with country codes
    Help,
    Version,
    Removed(String), // A removed command; the String is the hint to show
    Unknown(String),
}

// Hint for the removed '-quit' command (`prefix` is "-" on the command line, "" in the menu)
pub fn quit_removed_hint(prefix: &str) -> String {
    format!(
        "'{p}quit' was removed. Use '{p}stop' to stop CCaps now, or '{p}disable' to also remove it from auto-startup.",
        p = prefix
    )
}

pub fn parse_args() -> CliCommand {
    let args: Vec<String> = env::args().skip(1).collect();
    parse_command(&args)
}

// Parses the command line without the program name. No arguments: interactive menu.
fn parse_command(args: &[String]) -> CliCommand {
    let Some(command) = args.first() else {
        return CliCommand::Menu;
    };
    let country_codes = || parse_country_codes(&args[1..]);

    match command.as_str() {
        "-enable" => CliCommand::Enable(country_codes()),
        "-disable" => CliCommand::Disable,
        "-start" => CliCommand::Start(country_codes()),
        "-stop" => CliCommand::Stop,
        "-quit" => CliCommand::Removed(quit_removed_hint("-")),
        "-status" => CliCommand::Status,
        "-run" => CliCommand::Run(country_codes()),
        "--background" => CliCommand::Background(country_codes()),
        "-help" | "--help" | "-h" | "/?" => CliCommand::Help,
        "-v" | "--version" => CliCommand::Version,
        _ => CliCommand::Unknown(command.clone()),
    }
}

// Country codes from arguments like "-de -fr" (the leading dash is removed). Arguments
// without a dash and a lone "-" are ignored.
pub fn parse_country_codes<S: AsRef<str>>(args: &[S]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.as_ref())
        .filter(|arg| arg.starts_with('-') && arg.len() > 1)
        .map(|arg| arg[1..].to_string())
        .collect()
}

pub fn execute_command(command: CliCommand) -> (i32, Vec<String>) {
    match command {
        CliCommand::Enable(country_codes) => (handle_enable(&country_codes), vec![]),
        CliCommand::Disable => (handle_disable(), vec![]),
        CliCommand::Start(country_codes) => (handle_start(&country_codes), vec![]),
        CliCommand::Stop => (handle_stop(), vec![]),
        CliCommand::Status => (handle_status(), vec![]),
        // The background process doesn't touch auto-startup: only -enable/-disable do
        CliCommand::Background(country_codes) => (0, country_codes),
        CliCommand::Run(country_codes) => (0, country_codes), // Continue normal execution
        CliCommand::Menu => (0, vec![]),                      // This should not be called directly
        CliCommand::Help => {
            show_help();
            (0, vec![])
        }
        CliCommand::Version => {
            show_version();
            (0, vec![])
        }
        CliCommand::Removed(hint) => {
            eprintln!("{}", hint);
            (1, vec![])
        }
        CliCommand::Unknown(cmd) => {
            eprintln!("Unknown command: {}", cmd);
            (1, vec![])
        }
    }
}

// Status line and recommendation for -status, from whether the background process is
// running and whether it is in auto-startup
fn status_recommendation(running: bool, in_startup: bool) -> (&'static str, Option<&'static str>) {
    match (running, in_startup) {
        (true, true) => ("Status: All systems operational ✓", None),
        (true, false) => (
            "Status: Running, but not in auto-startup",
            Some("Recommendation: Run 'ccaps -enable' to start automatically at login"),
        ),
        (false, true) => (
            "Status: Auto-startup enabled but not currently running",
            Some("Recommendation: Run 'ccaps -start' to start it now"),
        ),
        (false, false) => (
            "Status: Not running and auto-startup disabled",
            Some("Recommendation: Run 'ccaps -enable' to start now and at every login"),
        ),
    }
}

// Why the saved country codes can't be used (e.g. a layout was removed from Windows
// after '-enable'), or None if they are fine. Checked before starting the background
// process, which would otherwise exit right away.
fn saved_codes_problem(saved: &[String]) -> Option<String> {
    if saved.is_empty() {
        return None;
    }
    let codes: Vec<&str> = saved.iter().map(|s| s.as_str()).collect();
    layout_manager::validate_country_codes(&codes)
        .err()
        .map(|error| {
            format!(
                "Error: the saved settings can't be used. {}\nSave new ones with 'ccaps -enable -xx', or start with 'ccaps -start -xx'.",
                error
            )
        })
}

// Validates country codes and prints which layouts will be used
fn check_country_codes(country_codes: &[String]) -> bool {
    if country_codes.is_empty() {
        println!("Using all available layouts");
        return true;
    }
    match layout_manager::validate_country_codes(
        &country_codes.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    ) {
        Ok(_) => {
            println!("Using country codes: {}", country_codes.join(", "));
            true
        }
        Err(error) => {
            eprintln!("Error: {}", error);
            false
        }
    }
}

fn handle_status() -> i32 {
    println!("CCaps Layout Switcher Status:");
    println!("╞══════════════════════════════════════════════════════════════╡");

    // Check if running in background
    let is_running = is_already_running();
    println!(
        "Background process: {}",
        if is_running {
            "RUNNING ✓"
        } else {
            "NOT RUNNING ✗"
        }
    );

    // Check startup entry
    let in_startup = is_in_startup();
    println!(
        "Auto-startup:       {}",
        if in_startup {
            "ENABLED ✓"
        } else {
            "DISABLED ✗"
        }
    );

    // Show startup path if enabled
    if in_startup {
        if let Ok(startup_path) = get_startup_path() {
            println!("Startup command:    {}", startup_path);
        }
    }

    // Show configuration info
    let config = config::load_config();
    let (config_exists, config_path) = config::get_config_info();
    println!(
        "Configuration file: {}",
        if config_exists {
            "EXISTS ✓"
        } else {
            "NOT FOUND ✗"
        }
    );
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

        println!(
            "│ -{:<2} │ {:<36} │ {:<15} │",
            layout.short_code, layout.name, status
        );
    }
    println!("└─────┴──────────────────────────────────────┴─────────────────┘");

    println!();
    println!("Usage examples:");
    println!("  ccaps -run            # Run in foreground mode (cycle through all layouts)");
    println!("  ccaps -run -de        # Switch between English and German");
    println!("  ccaps -run -de -fr    # Switch between German and French");
    println!("  ccaps -enable -de     # Run in background now and at every login (English/German)");
    println!("  ccaps -start          # Run in background now, without auto-startup");
    println!();

    // Show recommendations
    let (status, recommendation) = status_recommendation(is_running, in_startup);
    println!("{}", status);
    if let Some(recommendation) = recommendation {
        println!("{}", recommendation);
    }

    0
}

// -enable: save the configuration, add to auto-startup, and start the background
// process unless it is already running
fn handle_enable(country_codes: &[String]) -> i32 {
    println!("Enabling CCaps Layout Switcher...");

    if !check_country_codes(country_codes) {
        return 1;
    }

    // Save configuration
    let config = config::Config::with_country_codes(country_codes.to_vec());
    if let Err(e) = config::save_config(&config) {
        eprintln!("Warning: Could not save configuration: {}", e);
    } else {
        println!("Configuration saved.");
    }

    // Add to startup with country codes
    let already_in_startup = is_in_startup();
    if let Err(e) = add_to_startup(country_codes) {
        eprintln!("Warning: Could not add to startup: {}", e);
    } else if already_in_startup {
        println!("System startup configuration updated.");
    } else {
        println!("Added to system startup.");
    }

    if is_already_running() {
        println!(
            "Already running; new settings take effect after 'ccaps -stop' + 'ccaps -start' or next login."
        );
        return 0;
    }

    // Start in background (completely detached process)
    launch_background(country_codes)
}

// -start: start the background process now. Without country codes it uses the saved
// configuration (or all layouts); codes given here are not saved. Auto-startup is not
// changed.
fn handle_start(country_codes: &[String]) -> i32 {
    println!("Starting CCaps Layout Switcher...");

    // Check if already running
    if is_already_running() {
        println!("The program is already running in the background.");
        return 1;
    }

    if country_codes.is_empty() {
        let saved = config::load_config().country_codes;
        if let Some(problem) = saved_codes_problem(&saved) {
            eprintln!("{}", problem);
            return 1;
        }
        if saved.is_empty() {
            println!("Using all available layouts");
        } else {
            println!("Using saved country codes: {}", saved.join(", "));
        }
    } else if !check_country_codes(country_codes) {
        return 1;
    }

    // Start in background (completely detached process)
    let exit_code = launch_background(country_codes);
    if exit_code == 0 && !is_in_startup() {
        println!("It won't start at login; use 'ccaps -enable' for that.");
    }
    exit_code
}

fn ask_confirmation(prompt: &str) -> bool {
    ask_confirmation_with_reader(prompt, &mut io::stdin().lock())
}

fn ask_confirmation_with_reader<R: io::BufRead>(prompt: &str, reader: &mut R) -> bool {
    print!("{} [y/n]: ", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    match reader.read_line(&mut input) {
        Ok(_) => {
            let trimmed = input.trim().to_lowercase();
            trimmed == "y" || trimmed == "yes"
        }
        Err(_) => false,
    }
}

// -disable: stop the background process, remove it from auto-startup and delete the
// configuration
fn handle_disable() -> i32 {
    println!("Disabling CCaps Layout Switcher...");

    // Ask for confirmation
    if !ask_confirmation(
        "Are you sure you want to stop CCaps, remove it from startup and delete its settings?",
    ) {
        println!("Operation cancelled.");
        return 0;
    }

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

// -stop: stop the background process only; configuration and auto-startup are kept
fn handle_stop() -> i32 {
    println!("Stopping CCaps Layout Switcher...");

    // Ask for confirmation
    if !ask_confirmation("Are you sure you want to stop the background process?") {
        println!("Operation cancelled.");
        return 0;
    }

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
    println!("  ccaps                - Show interactive menu");
    println!("  ccaps -run           - Run in foreground mode (cycle through all layouts)");
    println!("  ccaps -run -de       - Run with English ↔ German switching");
    println!("  ccaps -run -de -fr   - Run with German ↔ French switching");
    println!();
    println!("  ccaps -enable        - Save settings, add to auto-startup and start now");
    println!("  ccaps -enable -de    - Same, with English ↔ German switching");
    println!("  ccaps -disable       - Stop, remove from auto-startup and delete settings");
    println!("  ccaps -start         - Start in background now (saved settings, no auto-startup)");
    println!("  ccaps -start -de     - Start in background now with English ↔ German (not saved)");
    println!(
        "  ccaps -stop          - Stop the background process (settings and auto-startup kept)"
    );
    println!();
    println!("  ccaps -status        - Show current status and available language codes");
    println!("  ccaps -help          - Show this help");
    println!("  ccaps -v             - Show version information");
    println!();
    println!("Note: unlike 'systemctl enable', '-enable' also starts CCaps right away.");
    println!();
    println!("Key bindings:");
    println!("  Caps Lock              - Switch keyboard layout");
    println!("  Shift + Caps Lock      - Toggle Caps Lock");
    println!("  Scroll Lock indicator  - Shows current layout (OFF=English, ON=Non-English)");
    println!();
    println!("Configuration:");
    println!("  Settings are saved by -enable and deleted by -disable");
    println!("  Configuration file: %localappdata%\\CCaps\\ccaps-config.json");
    println!("  Use 'ccaps -status' to see all available language codes");
    println!();
}

fn is_already_running() -> bool {
    unsafe {
        let mutex_name = format!("{}\0", MUTEX_NAME);
        let mutex_name_wide: Vec<u16> = OsString::from(mutex_name).encode_wide().collect();

        let mutex = CreateMutexW(ptr::null_mut(), FALSE, mutex_name_wide.as_ptr());

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
        let key_name_wide: Vec<u16> = OsString::from(key_name).encode_wide().collect();

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
        let key_name_wide: Vec<u16> = OsString::from(key_name).encode_wide().collect();

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
        let key_name_wide: Vec<u16> = OsString::from(key_name).encode_wide().collect();

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
        let exe_path = env::current_exe().map_err(|_| "Failed to get executable path")?;

        let mut exe_path_str = background_command_line(&exe_path, country_codes);
        exe_path_str.push('\0');

        let exe_path_wide: Vec<u16> = OsString::from(exe_path_str).encode_wide().collect();

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
        let key_name_wide: Vec<u16> = OsString::from(key_name).encode_wide().collect();

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

// Command line of the background process, as used for the Run key and for -start/-enable:
// the quoted executable path, "--background" and the country codes
fn background_command_line(exe_path: &Path, country_codes: &[String]) -> String {
    let mut command_line = format!("\"{}\" --background", exe_path.display());
    for code in country_codes {
        command_line.push_str(&format!(" -{}", code));
    }
    command_line
}

// Starts the background process detached from the caller.
//
// std::process::Command can't be used here: it always creates the child with handle
// inheritance on, so the child gets every inheritable handle of the caller, not only the
// standard ones. A script that captures the output of 'ccaps -start' would then wait
// forever (the background process keeps the pipe open), and a file the output was
// redirected to would stay locked. So the process is created without inheriting any
// handle and without standard handles (it detaches from the console anyway).
//
// The working directory is the executable's folder, so the background process doesn't
// keep the caller's current folder open (it couldn't be deleted or renamed).
fn start_background_process(country_codes: &[String]) -> Result<StartResult, String> {
    let exe_path = env::current_exe().map_err(|_| "Failed to get executable path")?;
    let working_dir = exe_path
        .parent()
        .ok_or("Failed to get executable folder")?
        .to_path_buf();

    let to_wide = |s: &OsStr| -> Vec<u16> { s.encode_wide().chain(std::iter::once(0)).collect() };
    let application_wide = to_wide(exe_path.as_os_str());
    // CreateProcessW may modify the command line buffer, so it must be mutable
    let mut command_line_wide = to_wide(OsStr::new(&background_command_line(
        &exe_path,
        country_codes,
    )));
    let working_dir_wide = to_wide(working_dir.as_os_str());

    unsafe {
        let mut startup_info: STARTUPINFOW = mem::zeroed();
        startup_info.cb = mem::size_of::<STARTUPINFOW>() as DWORD;
        let mut process_info: PROCESS_INFORMATION = mem::zeroed();

        let created = CreateProcessW(
            application_wide.as_ptr(),
            command_line_wide.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            FALSE, // inherit no handles
            CREATE_NO_WINDOW,
            ptr::null_mut(),
            working_dir_wide.as_ptr(),
            &mut startup_info,
            &mut process_info,
        );
        if created == 0 {
            return Err(format!("Failed to start process: error {}", GetLastError()));
        }
        CloseHandle(process_info.hThread);

        // Wait until the background process is ready (its window exists, which is also
        // what -stop looks for) or has exited, e.g. because of invalid saved codes
        let mut waited_ms = 0;
        let result = loop {
            let mut exit_code: DWORD = 0;
            let exited = WaitForSingleObject(process_info.hProcess, 0) == WAIT_OBJECT_0
                && GetExitCodeProcess(process_info.hProcess, &mut exit_code) != 0;
            let window_found = !find_background_window().is_null();
            if let Some(result) =
                start_wait_step(window_found, exited.then_some(exit_code), waited_ms)
            {
                break result;
            }
            std::thread::sleep(std::time::Duration::from_millis(START_POLL_MS));
            waited_ms += START_POLL_MS;
        };
        CloseHandle(process_info.hProcess);
        Ok(result)
    }
}

// How long -start/-enable wait for the background process to be ready
const START_TIMEOUT_MS: u64 = 3000;
const START_POLL_MS: u64 = 50;

#[derive(Debug, PartialEq)]
enum StartResult {
    // The background process is ready
    Running,
    // It exited right away with this code
    Exited(u32),
    // It neither got ready nor exited within START_TIMEOUT_MS
    StillStarting,
}

// One step of waiting for a started background process: None means keep waiting
fn start_wait_step(
    window_found: bool,
    exit_code: Option<u32>,
    waited_ms: u64,
) -> Option<StartResult> {
    if window_found {
        Some(StartResult::Running)
    } else if let Some(code) = exit_code {
        Some(StartResult::Exited(code))
    } else if waited_ms >= START_TIMEOUT_MS {
        Some(StartResult::StillStarting)
    } else {
        None
    }
}

// Starts the background process and reports the result; returns the exit code for
// -start/-enable
fn launch_background(country_codes: &[String]) -> i32 {
    match start_background_process(country_codes) {
        Ok(StartResult::Running) => {
            println!("CCaps Layout Switcher started in background.");
            0
        }
        Ok(StartResult::StillStarting) => {
            println!(
                "CCaps Layout Switcher is starting in background (not ready yet; check with 'ccaps -status')."
            );
            0
        }
        Ok(StartResult::Exited(code)) => {
            eprintln!(
                "The background process exited right away (code {}). Run 'ccaps -run' to see why.",
                code
            );
            1
        }
        Err(e) => {
            eprintln!("Failed to start background process: {}", e);
            1
        }
    }
}

// The hidden window of the running background process, or null if there is none
fn find_background_window() -> HWND {
    unsafe {
        FindWindowA(
            ptr::null(),
            b"CCaps Layout Switcher\0".as_ptr() as *const i8,
        )
    }
}

fn stop_background_process() -> bool {
    // Send quit message to running instance
    let window = find_background_window();
    if window.is_null() {
        return false;
    }
    unsafe {
        PostMessageA(window, WM_QUIT, 0, 0);
    }
    true
}

pub fn create_mutex() -> HANDLE {
    unsafe {
        let mutex_name = format!("{}\0", MUTEX_NAME);
        let mutex_name_wide: Vec<u16> = OsString::from(mutex_name).encode_wide().collect();

        CreateMutexW(ptr::null_mut(), TRUE, mutex_name_wide.as_ptr())
    }
}

pub fn should_run_in_background() -> bool {
    let args: Vec<String> = env::args().collect();
    args.len() > 1 && (args[1] == "--background")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse(args: &[&str]) -> CliCommand {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse_command(&args)
    }

    fn codes(codes: &[&str]) -> Vec<String> {
        codes.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_no_arguments_shows_menu() {
        assert_eq!(parse(&[]), CliCommand::Menu);
    }

    #[test]
    fn test_parse_enable() {
        assert_eq!(parse(&["-enable"]), CliCommand::Enable(vec![]));
        assert_eq!(
            parse(&["-enable", "-de", "-fr"]),
            CliCommand::Enable(codes(&["de", "fr"]))
        );
    }

    #[test]
    fn test_parse_disable() {
        assert_eq!(parse(&["-disable"]), CliCommand::Disable);
    }

    #[test]
    fn test_parse_start() {
        assert_eq!(parse(&["-start"]), CliCommand::Start(vec![]));
        assert_eq!(parse(&["-start", "-de"]), CliCommand::Start(codes(&["de"])));
    }

    #[test]
    fn test_stop_never_disables() {
        // Regression guard: '-stop' used to remove auto-startup and delete the config;
        // now it must only stop the process
        assert_eq!(parse(&["-stop"]), CliCommand::Stop);
        assert_ne!(parse(&["-stop"]), CliCommand::Disable);
    }

    #[test]
    fn test_parse_run_and_background() {
        assert_eq!(parse(&["-run", "-de"]), CliCommand::Run(codes(&["de"])));
        assert_eq!(
            parse(&["--background", "-ru"]),
            CliCommand::Background(codes(&["ru"]))
        );
    }

    #[test]
    fn test_quit_is_removed_with_hint() {
        match parse(&["-quit"]) {
            CliCommand::Removed(hint) => {
                assert!(hint.contains("'-stop'"));
                assert!(hint.contains("'-disable'"));
            }
            other => panic!("expected Removed, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_other_commands() {
        assert_eq!(parse(&["-status"]), CliCommand::Status);
        assert_eq!(parse(&["-help"]), CliCommand::Help);
        assert_eq!(parse(&["/?"]), CliCommand::Help);
        assert_eq!(parse(&["-v"]), CliCommand::Version);
        assert_eq!(
            parse(&["-bogus"]),
            CliCommand::Unknown("-bogus".to_string())
        );
    }

    #[test]
    fn test_country_codes_ignore_non_dash_args_and_lone_dash() {
        assert_eq!(
            parse_country_codes(&["-de", "fr", "-", "-ru"]),
            codes(&["de", "ru"])
        );
        assert!(parse_country_codes::<&str>(&[]).is_empty());
    }

    #[test]
    fn test_status_recommendation_all_states() {
        assert_eq!(status_recommendation(true, true).1, None);

        let (_, running_only) = status_recommendation(true, false);
        assert!(running_only.unwrap().contains("-enable"));

        let (_, startup_only) = status_recommendation(false, true);
        assert!(startup_only.unwrap().contains("-start"));

        let (_, neither) = status_recommendation(false, false);
        assert!(neither.unwrap().contains("-enable"));
    }

    #[test]
    fn test_no_saved_codes_is_fine() {
        // No saved settings: -start uses all layouts
        assert_eq!(saved_codes_problem(&[]), None);
    }

    #[test]
    fn test_unknown_saved_code_is_reported_before_start() {
        // 'zz' can't be a layout code: unknown languages get hexadecimal codes
        let problem = saved_codes_problem(&codes(&["zz"])).expect("zz must be rejected");
        assert!(problem.contains("zz"));
        assert!(problem.contains("-enable"));
    }

    #[test]
    fn test_start_waits_until_window_or_exit() {
        assert_eq!(start_wait_step(false, None, 0), None);
        assert_eq!(start_wait_step(false, None, START_TIMEOUT_MS - 1), None);
    }

    #[test]
    fn test_start_succeeds_when_window_appears() {
        // Regression: -start used to report success at once, so '-start; -stop' in a
        // script found no window and left the process running
        assert_eq!(start_wait_step(true, None, 100), Some(StartResult::Running));
    }

    #[test]
    fn test_start_reports_process_that_exited() {
        // e.g. the saved config names a layout that was removed from Windows
        assert_eq!(
            start_wait_step(false, Some(1), 100),
            Some(StartResult::Exited(1))
        );
    }

    #[test]
    fn test_start_gives_up_waiting_after_timeout() {
        assert_eq!(
            start_wait_step(false, None, START_TIMEOUT_MS),
            Some(StartResult::StillStarting)
        );
    }

    #[test]
    fn test_ready_window_wins_over_exit_code() {
        // Both seen in the same step: the window was there, so it did start
        assert_eq!(
            start_wait_step(true, Some(0), 100),
            Some(StartResult::Running)
        );
    }

    #[test]
    fn test_background_command_line_quotes_path_with_spaces() {
        let exe = Path::new(r"C:\Program Files\CCaps\ccaps.exe");
        assert_eq!(
            background_command_line(exe, &codes(&["de", "fr"])),
            r#""C:\Program Files\CCaps\ccaps.exe" --background -de -fr"#
        );
    }

    #[test]
    fn test_background_command_line_without_codes() {
        // No codes: the background process loads the saved configuration
        let exe = Path::new(r"C:\Users\me\bin\ccaps.exe");
        assert_eq!(
            background_command_line(exe, &[]),
            r#""C:\Users\me\bin\ccaps.exe" --background"#
        );
    }

    #[test]
    fn test_background_command_line_parses_back() {
        // What -start/-enable launch must parse as the internal background command
        let exe = Path::new(r"C:\Program Files\CCaps\ccaps.exe");
        let command_line = background_command_line(exe, &codes(&["ru"]));
        let args: Vec<String> = command_line
            .rsplit('"')
            .next()
            .unwrap()
            .split_whitespace()
            .map(String::from)
            .collect();
        assert_eq!(parse_command(&args), CliCommand::Background(codes(&["ru"])));
    }

    #[test]
    fn test_ask_confirmation_yes() {
        let input = "y\n";
        let mut reader = Cursor::new(input);
        let result = ask_confirmation_with_reader("Test prompt", &mut reader);
        assert!(result, "Expected confirmation with 'y' to return true");
    }

    #[test]
    fn test_ask_confirmation_yes_uppercase() {
        let input = "Y\n";
        let mut reader = Cursor::new(input);
        let result = ask_confirmation_with_reader("Test prompt", &mut reader);
        assert!(result, "Expected confirmation with 'Y' to return true");
    }

    #[test]
    fn test_ask_confirmation_yes_full() {
        let input = "yes\n";
        let mut reader = Cursor::new(input);
        let result = ask_confirmation_with_reader("Test prompt", &mut reader);
        assert!(result, "Expected confirmation with 'yes' to return true");
    }

    #[test]
    fn test_ask_confirmation_yes_full_uppercase() {
        let input = "YES\n";
        let mut reader = Cursor::new(input);
        let result = ask_confirmation_with_reader("Test prompt", &mut reader);
        assert!(result, "Expected confirmation with 'YES' to return true");
    }

    #[test]
    fn test_ask_confirmation_no() {
        let input = "n\n";
        let mut reader = Cursor::new(input);
        let result = ask_confirmation_with_reader("Test prompt", &mut reader);
        assert!(!result, "Expected confirmation with 'n' to return false");
    }

    #[test]
    fn test_ask_confirmation_no_uppercase() {
        let input = "N\n";
        let mut reader = Cursor::new(input);
        let result = ask_confirmation_with_reader("Test prompt", &mut reader);
        assert!(!result, "Expected confirmation with 'N' to return false");
    }

    #[test]
    fn test_ask_confirmation_no_full() {
        let input = "no\n";
        let mut reader = Cursor::new(input);
        let result = ask_confirmation_with_reader("Test prompt", &mut reader);
        assert!(!result, "Expected confirmation with 'no' to return false");
    }

    #[test]
    fn test_ask_confirmation_invalid_input() {
        let input = "maybe\n";
        let mut reader = Cursor::new(input);
        let result = ask_confirmation_with_reader("Test prompt", &mut reader);
        assert!(
            !result,
            "Expected confirmation with invalid input to return false"
        );
    }

    #[test]
    fn test_ask_confirmation_empty_input() {
        let input = "\n";
        let mut reader = Cursor::new(input);
        let result = ask_confirmation_with_reader("Test prompt", &mut reader);
        assert!(
            !result,
            "Expected confirmation with empty input to return false"
        );
    }

    #[test]
    fn test_ask_confirmation_with_whitespace() {
        let input = "  y  \n";
        let mut reader = Cursor::new(input);
        let result = ask_confirmation_with_reader("Test prompt", &mut reader);
        assert!(
            result,
            "Expected confirmation with 'y' surrounded by whitespace to return true"
        );
    }
}
