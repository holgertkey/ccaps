use crate::cli::{execute_command, parse_country_codes, quit_removed_hint, CliCommand};
use crate::layout_manager;
use std::io::{self, Write};

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
                            println!();
                            println!("Starting in foreground mode with all layouts...");
                        } else {
                            println!();
                            println!(
                                "Starting in foreground mode with country codes: {}",
                                country_codes.join(", ")
                            );
                        }
                        // Don't install Ctrl+C handler here - let main.rs handle it
                        return (0, country_codes); // Return country codes to main
                    }
                    CliCommand::Help => {
                        println!();
                        let (result, _) = execute_command(command);
                        if result != 0 {
                            println!("Command failed with code: {}", result);
                        }
                        println!();
                    }
                    CliCommand::Status => {
                        println!();
                        let (result, _) = execute_command(command);
                        if result != 0 {
                            println!("Command failed with code: {}", result);
                        }
                        println!();
                    }
                    CliCommand::Unknown(ref cmd) if cmd == "menu" => {
                        println!();
                        show_menu(); // Show menu again
                        println!();
                    }
                    CliCommand::Unknown(ref cmd) if cmd == "exit" => {
                        println!();
                        println!("Goodbye!");
                        return (1, vec![]);
                    }
                    CliCommand::Removed(ref hint) => {
                        println!();
                        println!("{}", hint);
                        println!();
                    }
                    CliCommand::Unknown(ref cmd) if cmd.starts_with("Invalid codes:") => {
                        // Don't execute invalid commands, just continue
                        println!();
                    }
                    _ => {
                        println!();
                        let (result, _) = execute_command(command);
                        if result != 0 {
                            println!("Command failed with code: {}", result);
                        }
                        println!();
                    }
                }
            }
            Err(error) => {
                eprintln!("Error reading input: {}", error);
                return (1, vec![]);
            }
        }
    }
}

fn show_status() {
    let version = env!("CARGO_PKG_VERSION");
    let title = format!("CCaps Layout Switcher v{}", version);
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║{:^78}║", title);
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
    println!("│  run -de       - Run with English ↔ German switching                       │");
    println!("│  run -de -fr   - Run with German ↔ French switching                        │");
    println!("│  enable        - Save settings, auto-startup, start now (all layouts)      │");
    println!("│  enable -de    - Same, with English ↔ German switching                     │");
    println!("│  disable       - Stop, remove from auto-startup and delete settings        │");
    println!("│  start         - Start in background now (no auto-startup)                 │");
    println!("│  stop          - Stop background process (settings and auto-startup kept)  │");
    println!("│  status        - Show current status and available language codes          │");
    println!("│  help          - Show detailed help                                        │");
    println!("│  menu          - Show this menu again                                      │");
    println!("│  exit (e)      - Exit this menu                                            │");
    println!("└────────────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("Key bindings when running:");
    println!("  Caps Lock              - Switch keyboard layout");
    println!("  Shift + Caps Lock      - Toggle Caps Lock");
    println!("  Scroll Lock indicator  - Shows current layout (OFF=English, ON=Non-English)");
    println!();
}

fn parse_menu_command(input: &str) -> CliCommand {
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() {
        return CliCommand::Unknown(input.to_string());
    }

    // Country codes after run/enable/start, validated against the installed layouts
    let validated_codes = || -> Result<Vec<String>, CliCommand> {
        let country_codes = parse_country_codes(&parts[1..]);
        println!();
        if country_codes.is_empty() {
            println!("✓ Using all available layouts");
            return Ok(country_codes);
        }
        match layout_manager::validate_country_codes(
            &country_codes.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        ) {
            Ok(_) => {
                println!("✓ Validated country codes: {}", country_codes.join(", "));
                Ok(country_codes)
            }
            Err(error) => {
                println!("✗ Error: {}", error);
                Err(CliCommand::Unknown(format!("Invalid codes: {}", input)))
            }
        }
    };

    match parts[0].to_lowercase().as_str() {
        "run" => validated_codes().map_or_else(|e| e, CliCommand::Run),
        "enable" => validated_codes().map_or_else(|e| e, CliCommand::Enable),
        "start" => validated_codes().map_or_else(|e| e, CliCommand::Start),
        "disable" => CliCommand::Disable,
        "stop" => CliCommand::Stop,
        "quit" => CliCommand::Removed(quit_removed_hint("")),
        "status" => CliCommand::Status,
        "help" => CliCommand::Help,
        "menu" => CliCommand::Unknown("menu".to_string()),
        "exit" | "e" => CliCommand::Unknown("exit".to_string()),
        _ => CliCommand::Unknown(input.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_commands_without_codes() {
        assert_eq!(parse_menu_command("run"), CliCommand::Run(vec![]));
        assert_eq!(parse_menu_command("enable"), CliCommand::Enable(vec![]));
        assert_eq!(parse_menu_command("start"), CliCommand::Start(vec![]));
        assert_eq!(parse_menu_command("disable"), CliCommand::Disable);
        assert_eq!(parse_menu_command("stop"), CliCommand::Stop);
        assert_eq!(parse_menu_command("status"), CliCommand::Status);
        assert_eq!(parse_menu_command("help"), CliCommand::Help);
    }

    #[test]
    fn test_menu_commands_are_case_insensitive() {
        assert_eq!(parse_menu_command("STOP"), CliCommand::Stop);
        assert_eq!(parse_menu_command("Enable"), CliCommand::Enable(vec![]));
    }

    #[test]
    fn test_menu_stop_never_disables() {
        assert_ne!(parse_menu_command("stop"), CliCommand::Disable);
    }

    #[test]
    fn test_menu_quit_is_removed_with_hint() {
        match parse_menu_command("quit") {
            CliCommand::Removed(hint) => {
                assert!(hint.contains("'stop'"));
                assert!(hint.contains("'disable'"));
            }
            other => panic!("expected Removed, got {:?}", other),
        }
    }

    #[test]
    fn test_menu_exit_leaves_the_menu() {
        assert_eq!(
            parse_menu_command("e"),
            CliCommand::Unknown("exit".to_string())
        );
        assert_eq!(
            parse_menu_command("exit"),
            CliCommand::Unknown("exit".to_string())
        );
    }
}
