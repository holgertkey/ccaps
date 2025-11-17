# CCaps Layout Switcher v0.7.0

A lightweight Windows keyboard layout switcher that repurposes the Caps Lock key for quick layout switching with country-specific filtering and configuration persistence.

## Features

- **Caps Lock → Layout Switch**: Press Caps Lock to cycle through keyboard layouts
- **Country Code Filtering**: Choose specific layouts to switch between (e.g., English ↔ German)
- **Alt + Caps Lock → Caps Lock**: Hold Alt and press Caps Lock to toggle Caps Lock functionality
- **Visual Indicator**: Scroll Lock LED shows current layout (OFF = English, ON = Non-English)
- **Background Mode**: Runs silently in the background
- **Auto-startup**: Automatically starts with Windows
- **Configuration Persistence**: Remembers your layout preferences
- **Low Resource Usage**: Minimal CPU and memory footprint
- **No Dependencies**: Single executable file

## Installation

### Option 1: Install from crates.io (Recommended)

```bash
cargo install ccaps
```

This will download, compile, and install the latest version of CCaps. The executable will be placed in your Cargo bin directory (usually `~/.cargo/bin/` or `%USERPROFILE%\.cargo\bin\`).

### Option 2: Download Pre-built Binary

1. Download the latest release from the [Releases](../../releases) page
2. Extract `ccaps.exe` to any folder (e.g., `C:\Program Files\CCaps\`)
3. Run the program using command line options

## Usage

### Command Line Options

```bash
# Basic commands
ccaps              # Show interactive menu
ccaps -run         # Run in foreground mode (all layouts)
ccaps -start       # Start in background with all layouts + add to auto-startup
ccaps -start -de   # Start in background with English/German + add to auto-startup
ccaps -stop        # Stop background process + remove from startup + delete config
ccaps -exit        # Stop background process only
ccaps -status      # Show status and available language codes
ccaps -help        # Show help information
ccaps -v           # Show version information

# Country-specific switching
ccaps -run -de     # English ↔ German switching
ccaps -run -de -fr # German ↔ French switching (no English)
```

### Country Codes

Use `ccaps -status` to see all available language codes for your system. Common codes include:

| Code | Language       | Code | Language   | Code | Language   |
|------|----------------|------|------------|------|------------|
| `us` | English (US)   | `ru` | Russian    | `ua` | Ukrainian  |
| `gb` | English (UK)   | `de` | German     | `fr` | French     |
| `es` | Spanish        | `it` | Italian    | `pl` | Polish     |
| `pt` | Portuguese     | `nl` | Dutch      | `cz` | Czech      |
| `jp` | Japanese       | `kr` | Korean     | `cn` | Chinese    |

### Key Bindings

| Key Combination   | Action                          |
|-------------------|---------------------------------|
| `Caps Lock`       | Switch to next keyboard layout  |
| `Alt + Caps Lock` | Toggle Caps Lock on/off         |

### Visual Indicator

The Scroll Lock LED on your keyboard serves as a layout indicator:
- **OFF** (🔴) = English layout active
- **ON** (🟢) = Non-English layout active

## Quick Start Examples

### 1. Interactive Menu
```bash
ccaps
```
Shows a menu with all available options and current system status.

**Available commands in interactive mode:**
- `run` - Run in foreground mode (all layouts)
- `run -de` - Run with specific layouts (e.g., English ↔ German)
- `start` - Start in background with all layouts and add to auto-startup
- `start -de` - Start in background with specific layouts and auto-startup
- `stop` - Stop background process and remove from startup
- `exit` - Stop background process only
- `status` - Show current status and available language codes
- `help` - Show detailed help
- `menu` - Show menu again
- `quit` or `q` - Exit interactive menu

### 2. Switch Between English and German
```bash
ccaps -run -de
```

### 3. Switch Between Multiple Languages
```bash
ccaps -run -de -fr -es  # German ↔ French ↔ Spanish
```

### 4. Start in Background with English/German and Auto-startup
```bash
ccaps -start -de
```

### 5. Start in Background with All Layouts and Auto-startup
```bash
ccaps -start
```

### 6. Check Available Languages and Current Configuration
```bash
ccaps -status
```
Output example:
```
CCaps Layout Switcher Status:
╞══════════════════════════════════════════════════════════════╡
Background process: RUNNING ✓
Auto-startup:       ENABLED ✓
Startup command:    "C:\Program Files\CCaps\ccaps.exe" --background -de
Configuration file: EXISTS ✓
Config path:        C:\Program Files\CCaps\ccaps-config.json
Saved country codes: de

Available keyboard layouts:
┌─────┬──────────────────────────────────────┬─────────────────┐
│ Code│ Language                             │ Status          │
├─────┼──────────────────────────────────────┼─────────────────┤
│ -us │ English (United States)              │ CURRENT ✓       │
│ -ru │ Russian                              │ Available       │
│ -ua │ Ukrainian                            │ Available       │
│ -de │ German                               │ Available       │
└─────┴──────────────────────────────────────┴─────────────────┘

Usage examples:
  ccaps -run            # Run in foreground mode (cycle through all layouts)
  ccaps -run -de        # Switch between English and German
  ccaps -start          # Start with all layouts and add to auto-startup  
  ccaps -start -de      # Start with English/German and add to auto-startup

Status: All systems operational ✓
```

## Configuration Persistence

CCaps automatically saves your layout preferences when using `-start` with country codes:

- **Configuration file**: `ccaps-config.json` (stored in `%LOCALAPPDATA%\CCaps\`)
- **Typical location**: `C:\Users\<username>\AppData\Local\CCaps\ccaps-config.json`
- **Auto-restore**: Background process automatically loads saved preferences
- **JSON format**: Human-readable configuration file

Example configuration file:
```json
{
  "country_codes": ["de"],
  "version": "0.7.0"
}
```

### Configuration Management

- **Automatic saving**: Using `ccaps -start -de` saves English/German preference
- **Auto-loading**: Background process loads saved preferences on Windows startup
- **Manual cleanup**: `ccaps -stop` removes configuration file
- **Status check**: `ccaps -status` shows current configuration

## How It Works

CCaps uses Windows low-level keyboard hooks to intercept Caps Lock key presses and redirect them to layout switching functionality. The program:

1. Installs a system-wide keyboard hook
2. Intercepts Caps Lock key events
3. Cycles through selected keyboard layouts (filtered by country codes)
4. Updates the Scroll Lock indicator to show the current layout
5. Blocks the default Caps Lock behavior (unless Alt is held)
6. Saves and restores layout preferences automatically

### Layout Selection Logic

- **No country codes**: Cycles through all installed layouts
- **One country code**: Switches between English and the specified language
- **Multiple country codes**: Cycles through the specified languages only
- **English preference**: If multiple layouts are specified, English is automatically included unless all specified layouts are non-English

## Supported Languages

The layout detection works with all Windows keyboard layouts. The program automatically detects over 40 languages including:

- **English variants**: US, UK, Australia, Canada, New Zealand, Ireland, South Africa
- **Cyrillic**: Russian, Ukrainian, Bulgarian, Serbian, Belarusian
- **Western European**: German, French, Spanish, Italian, Portuguese, Dutch
- **Nordic**: Norwegian, Swedish, Danish, Finnish, Icelandic
- **Eastern European**: Polish, Czech, Hungarian, Slovak, Romanian
- **Asian**: Japanese, Korean, Chinese (Simplified/Traditional), Thai, Vietnamese
- **Middle Eastern**: Arabic, Hebrew, Farsi

## Advanced Usage

### Background Process Management with Specific Layouts
```bash
# Start with specific layouts and auto-startup
ccaps -start -de          # English/German switching
ccaps -start -de -fr      # German/French switching
ccaps -start              # All layouts (default)

# The configuration is automatically saved and restored
```

### Interactive Menu with Configuration
```bash
ccaps
# Choose from menu:
# start -de     # This saves the preference and starts background process
# run -de       # This only runs temporarily without saving
# q             # Quick exit from interactive menu
```

### Registry Integration
The program stores startup configuration in:
```
HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
Key: "CCaps Layout Switcher"
Value: "C:\Program Files\CCaps\ccaps.exe" --background -de
```

### Status Monitoring
```bash
ccaps -status
```
Shows:
- Background process status
- Auto-startup configuration
- Configuration file status and location
- Saved country codes
- All available keyboard layouts with country codes
- Current active layout
- Usage examples and recommendations

## Building from Source

### Prerequisites

- Rust 1.70 or later
- Windows 10/11
- Visual Studio Build Tools (for linking)

### Build Steps

```bash
git clone https://github.com/holgertkey/ccaps.git
cd ccaps
cargo build --release
```

The executable will be created at `target/release/ccaps.exe`.

### Dependencies

- **winapi**: Windows API bindings
- **ctrlc**: Ctrl+C signal handling
- **serde**: Serialization framework
- **serde_json**: JSON serialization

## Technical Details

- **Language**: Rust
- **Version**: 0.7.0
- **Windows APIs**: WinAPI (winuser, winreg, synchapi, fileapi)
- **Hook Type**: Low-level keyboard hook (WH_KEYBOARD_LL)
- **Registry**: Uses `HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`
- **Configuration**: JSON file in `%LOCALAPPDATA%\CCaps\`
- **Mutex**: Global mutex prevents multiple instances
- **Layout Detection**: Language ID extraction from HKL handles

## Troubleshooting

### Invalid Country Code Error
```bash
ccaps -run -zz
# Error: Unknown country codes: zz. Use 'ccaps -status' to see available codes.
```
Solution: Run `ccaps -status` to see all available country codes for your system.

### Program doesn't start with Windows
```bash
# Check status
ccaps -status

# Restart and re-enable startup
ccaps -stop
ccaps -start -de    # or your preferred layout codes
```

### Configuration not loading
- Check if configuration file exists: `ccaps -status`
- Restart background process: `ccaps -exit` then `ccaps -start`
- Manually delete and recreate: `ccaps -stop` then `ccaps -start -de`

### Layout switching not working with specific codes
- Ensure the specified keyboard layouts are installed in Windows
- Check available codes with: `ccaps -status`
- Verify layouts in Settings → Time & Language → Language → Preferred languages

## Uninstall

```bash
# Stop the program and remove all traces
ccaps -stop

# Delete the executable file
del ccaps.exe

# Configuration file is automatically deleted by 'ccaps -stop'
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Changelog

### v0.7.0
- 🐛 Fixed sporadic Caps Lock LED activation during window switching and system startup
- 🔧 Improved Alt key state detection using real-time polling instead of event tracking
- 🔧 Removed dependency on Alt key state caching to prevent desynchronization
- ✨ Added confirmation prompts [y/n] for `-exit` and `-stop` commands to prevent accidental termination
- ✨ Added startup check in `-start` command to detect existing auto-startup entries
- 🔧 Improved user feedback messages when running `-start` (shows "Updating configuration..." vs "Added to startup")
- ✅ Added comprehensive unit tests for confirmation functionality (10 test cases)
- 📁 Moved configuration file to AppData directory (`%LOCALAPPDATA%\CCaps\ccaps-config.json`)
- 🐛 Fixed terminal minimizing issue when running `-start` command (now uses `CREATE_NO_WINDOW` flag)
- ✨ Added `q` command as a shortcut for `quit` in interactive menu

### v0.6.0
- ✨ Added configuration persistence with JSON file
- ✨ Enhanced `-start` command to accept country codes
- ✨ Background process now remembers layout preferences
- ✨ Interactive menu supports `start` command with country codes
- ✨ Improved status command with configuration information
- 🔧 Automatic configuration loading in background mode
- 🔧 Configuration cleanup on `-stop` command
- 📚 Updated documentation with configuration examples
- 🔧 Enhanced error handling for configuration management

### v0.5.0
- ✨ Added country code filtering for specific language switching
- ✨ Enhanced status command with layout table and usage examples
- ✨ Improved interactive menu with country code support
- ✨ Extended language detection (40+ languages supported)
- ✨ Smart layout selection logic with English preference
- 🔧 Better error handling for invalid country codes
- 📚 Comprehensive documentation updates

### v0.4.0
- 🎯 Initial release with basic layout switching
- ⚡ Low-level keyboard hook implementation
- 🔄 Auto-startup functionality
- 💡 Scroll Lock LED indicator
- 📱 Background process support
