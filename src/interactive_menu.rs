use std::io::{self, Write};
use crate::cli::{execute_command, CliCommand};
use crate::layout_manager;

pub fn show_interactive_menu() -> (i32, Vec<String>) {
    // Show initial menu only once
    show_status();
    show_menu();
    
    // No Ctrl+C handler in menu - let main.rs handle it later
    loop {
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
                    CliCommand::Run(country_codes) => {
                        if country_codes.is_empty() {
                            println!("Starting in foreground mode with all layouts...");
                        } else {
                            println!("Starting in foreground mode with country codes: {}", country_codes.join(", "));
                        }
                        println!(); // Extra line for better readability
                        // Don't install Ctrl+C handler here - let main.rs handle it
                        return (0, country_codes); // Return country codes to main
                    },
                    CliCommand::Help => {
                        println!();
                        let (result, _) = execute_command(command);
                        if result != 0 {
                            println!("Command failed with code: {}", result);
                        }
                        println!();
                    },
                    CliCommand::Status => {
                        println!();
                        let (result, _) = execute_command(command);
                        if result != 0 {
                            println!("Command failed with code: {}", result);
                        }
                        println!();
                    },
                    CliCommand::Unknown(ref cmd) if cmd == "menu" => {
                        println!();
                        show_menu(); // Show menu again
                        println!();
                    },
                    CliCommand::Unknown(ref cmd) if cmd == "quit" => {
                        println!("Goodbye!");
                        return (1, vec![]);
                    },
                    CliCommand::Unknown(ref cmd) if cmd.starts_with("Invalid codes:") => {
                        // Don't execute invalid commands, just continue
                        println!();
                    },
                    _ => {
                        println!();
                        let (result, _) = execute_command(command);
                        if result != 0 {
                            println!("Command failed with code: {}", result);
                        }
                        println!();
                    }
                }
            },
            Err(error) => {
                eprintln!("Error reading input: {}", error);
                return (1, vec![]);
            }
        }
    }
}

fn show_status() {
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                        CCaps Layout Switcher v0.5.0                          ║");
    println!("║                 Keyboard layout switcher using Caps Lock key                 ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
    println!();
    
    // Show current status
    let (result, _) = execute_command(CliCommand::Status);
    if result != 0 {
        println!("Warning: Could not retrieve full status");
    }
    println!();
}

fn show_menu() {
    println!("Available commands:");
    println!("┌────────────────────────────────────────────────────────────────────────────┐");
    println!("│  run           - Run in foreground mode (all layouts)                      │");
    println!("│  run -ru       - Run with English ↔ Russian switching                      │");
    println!("│  run -ua       - Run with English ↔ Ukrainian switching                    │");
    println!("│  run -de -fr   - Run with German ↔ French switching                        │");
    println!("│  start         - Start in background and add to system startup             │");
    println!("│  stop          - Stop background process and remove from startup           │");
    println!("│  exit          - Stop background process only                              │");
    println!("│  status        - Show current status and available language codes          │");
    println!("│  help          - Show detailed help                                        │");
    println!("│  menu          - Show this menu again                                      │");
    println!("│  quit          - Exit this menu                                            │");
    println!("└────────────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("Key bindings when running:");
    println!("  Caps Lock              - Switch keyboard layout");
    println!("  Alt + Caps Lock        - Toggle Caps Lock");
    println!("  Scroll Lock indicator  - Shows current layout (OFF=English, ON=Non-English)");
    println!();
}

fn parse_menu_command(input: &str) -> CliCommand {
    let parts: Vec<&str> = input.split_whitespace().collect();
    
    if parts.is_empty() {
        return CliCommand::Unknown(input.to_string());
    }
    
    match parts[0].to_lowercase().as_str() {
        "run" => {
            // Parse country codes after run command
            let country_codes: Vec<String> = parts[1..].iter()
                .filter(|arg| arg.starts_with('-') && arg.len() > 1)
                .map(|arg| arg[1..].to_string())
                .collect();
            
            // Validate country codes if provided
            if !country_codes.is_empty() {
                match layout_manager::validate_country_codes(
                    &country_codes.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                ) {
                    Ok(_) => {
                        println!("✓ Validated country codes: {}", country_codes.join(", "));
                    },
                    Err(error) => {
                        println!("✗ Error: {}", error);
                        return CliCommand::Unknown(format!("Invalid codes: {}", input));
                    }
                }
            } else {
                println!("✓ Using all available layouts");
            }
            
            CliCommand::Run(country_codes)
        },
        "start" => CliCommand::Start,
        "stop" => CliCommand::Stop,
        "exit" => CliCommand::Exit,
        "status" => CliCommand::Status,
        "help" => CliCommand::Help,
        "menu" => CliCommand::Unknown("menu".to_string()),
        "quit" => CliCommand::Unknown("quit".to_string()),
        _ => CliCommand::Unknown(input.to_string()),
    }
}