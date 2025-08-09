use std::ptr;
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use winapi::um::winuser::*;
use winapi::um::processthreadsapi::*;
use winapi::um::winbase::*;
use winapi::um::handleapi::*;
use winapi::um::errhandlingapi::GetLastError;
use winapi::shared::minwindef::*;

// Альтернативный способ создания полностью независимого процесса
pub unsafe fn create_detached_process(exe_path: &str, args: &str) -> Result<(), String> {
    let cmdline = format!("\"{}\" {}", exe_path, args);
    let mut cmdline_wide: Vec<u16> = cmdline
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    
    let mut startup_info: STARTUPINFOW = std::mem::zeroed();
    startup_info.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    startup_info.dwFlags = STARTF_USESHOWWINDOW | STARTF_USESTDHANDLES;
    startup_info.wShowWindow = SW_HIDE as u16;
    
    // Закрываем стандартные хендлы для полной отвязки
    startup_info.hStdInput = INVALID_HANDLE_VALUE;
    startup_info.hStdOutput = INVALID_HANDLE_VALUE;
    startup_info.hStdError = INVALID_HANDLE_VALUE;
    
    let mut process_info: PROCESS_INFORMATION = std::mem::zeroed();
    
    // Создание процесса с полной отвязкой
    let creation_flags = 
        CREATE_NEW_CONSOLE |        // Новая консоль
        DETACHED_PROCESS |          // Отвязанный процесс
        CREATE_NEW_PROCESS_GROUP |  // Новая группа процессов
        CREATE_BREAKAWAY_FROM_JOB;  // Выход из job object родителя
    
    let success = CreateProcessW(
        ptr::null(),
        cmdline_wide.as_mut_ptr(),
        ptr::null_mut(),
        ptr::null_mut(),
        FALSE, // Не наследуем хендлы
        creation_flags,
        ptr::null_mut(),
        ptr::null(),
        &mut startup_info,
        &mut process_info,
    );
    
    if success == 0 {
        return Err(format!("CreateProcessW failed with error: {}", GetLastError()));
    }
    
    // Немедленно закрываем хендлы для полной отвязки
    CloseHandle(process_info.hProcess);
    CloseHandle(process_info.hThread);
    
    Ok(())
}

// Функция для запуска через ShellExecute (еще один способ)
pub unsafe fn shell_execute_detached(exe_path: &str) -> Result<(), String> {
    use winapi::um::shellapi::ShellExecuteW;
    use winapi::um::winuser::SW_HIDE;
    
    let exe_path_wide: Vec<u16> = exe_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    
    let parameters_wide: Vec<u16> = "--background"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    
    let result = ShellExecuteW(
        ptr::null_mut(),
        ptr::null(),
        exe_path_wide.as_ptr(),
        parameters_wide.as_ptr(),
        ptr::null(),
        SW_HIDE,
    );
    
    if result as isize <= 32 {
        return Err(format!("ShellExecuteW failed with error: {}", result as isize));
    }
    
    Ok(())
}

// Функция для проверки и отвязки от консоли
pub unsafe fn ensure_detached_from_console() {
    let console_window = GetConsoleWindow();
    
    if !console_window.is_null() {
        // Есть консольное окно - отвязываемся от него
        FreeConsole();
        
        // Перенаправляем стандартные потоки в /dev/null эквивалент
        let null_file = CreateFileW(
            "NUL\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );
        
        if null_file != INVALID_HANDLE_VALUE {
            SetStdHandle(STD_OUTPUT_HANDLE, null_file);
            SetStdHandle(STD_ERROR_HANDLE, null_file);
            SetStdHandle(STD_INPUT_HANDLE, null_file);
        }
    }
}

// Функция для создания daemon-процесса по Unix-принципам, адаптированная для Windows
pub unsafe fn daemonize() -> Result<(), String> {
    // 1. Отвязываемся от консоли
    ensure_detached_from_console();
    
    // 2. Устанавливаем корневую директорию как рабочую (опционально)
    // SetCurrentDirectoryW("C:\\\0".encode_utf16().collect::<Vec<u16>>().as_ptr());