use std::io::{self, Write};
use crate::cli::{execute_command, CliCommand};

pub fn show_interactive_menu() -> i32 {
    // Set up Ctrl+C handler for menu only
    let original_handler = ctrlc::set_handler(move || {
        println!("\nExiting...");
        std::process::exit(0);
    });
    
    if let Err(e) = original_handler {
        eprintln!("Warning: Could not set Ctrl+C handler: {}", e);
    }

    loop {
        show_status();
        show_menu();
        
        print!("Enter command: ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let input = input.trim();
                
                if input.is_empty() {
                    continue;
                }
                
                let command = parse_menu_command(input);
                
                match command {
                    CliCommand::Run => {
                        println!("Starting in foreground mode...");
                        // Don't install Ctrl+C handler here - let main.rs handle it
                        return 0; // Return to main execution
                    },
                    CliCommand::Help => {
                        execute_command(command);
                        println!();
                    },
                    CliCommand::Unknown(ref cmd) if cmd == "exit" || cmd == "quit" => {
                        println!("Goodbye!");
                        return 1;
                    },
                    _ => {
                        let result = execute_command(command);
                        if result != 0 {
                            println!("Command failed with code: {}", result);
                        }
                        println!();
                    }
                }
            },
            Err(error) => {
                eprintln!("Error reading input: {}", error);
                return 1;
            }
        }
    }
}

fn show_status() {
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                            CCaps Layout Switcher v0.4.0                      ║");
    println!("║                        Keyboard layout switcher using Caps Lock key          ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
    println!();
    
    // Show current status
    execute_command(CliCommand::Status);
    println!();
}

fn show_menu() {
    println!("Available commands:");
    println!("┌────────────────────────────────────────────────────────────────────────────┐");
    println!("│  run     - Run in foreground mode                                          │");
    println!("│  start   - Start in background and add to system startup                   │");
    println!("│  stop    - Stop background process and remove from startup                 │");
    println!("│  exit    - Stop background process only                                    │");
    println!("│  status  - Show current status                                             │");
    println!("│  help    - Show detailed help                                              │");
    println!("│  quit    - Exit this menu                                                  │");
    println!("└────────────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("Key bindings when running:");
    println!("  Caps Lock              - Switch keyboard layout");
    println!("  Alt + Caps Lock        - Toggle Caps Lock");
    println!("  Scroll Lock indicator  - Shows current layout (OFF=English, ON=Non-English)");
    println!();
    println!("Use Ctrl+C to exit at any time");
    println!();
}

fn parse_menu_command(input: &str) -> CliCommand {
    match input.to_lowercase().as_str() {
        "run" => CliCommand::Run,
        "start" => CliCommand::Start,
        "stop" => CliCommand::Stop,
        "exit" => CliCommand::Exit,
        "status" => CliCommand::Status,
        "help" => CliCommand::Help,
        "quit" | "q" => CliCommand::Unknown("exit".to_string()),
        _ => CliCommand::Unknown(input.to_string()),
    }
}