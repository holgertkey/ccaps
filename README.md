# CCaps Layout Switcher v0.11.0

A lightweight Windows keyboard layout switcher that repurposes the Caps Lock key for quick layout switching with country-specific filtering and configuration persistence.

## Features

- **Caps Lock → Layout Switch**: Press Caps Lock to cycle through keyboard layouts
- **Country Code Filtering**: Choose specific layouts to switch between (e.g., English ↔ German)
- **Shift + Caps Lock → Caps Lock**: Hold Shift and press Caps Lock to toggle Caps Lock functionality
- **Visual Indicator**: Scroll Lock LED shows current layout (OFF = English, ON = Non-English)
- **Verified Switching**: Checks that the window actually switched, and falls back to Win+Space in applications that ignore the layout request
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
ccaps -enable      # Save settings + add to auto-startup + start now (all layouts)
ccaps -enable -de  # Same, with English/German
ccaps -disable     # Stop + remove from auto-startup + delete settings
ccaps -start       # Start in background now (saved settings), auto-startup untouched
ccaps -start -de   # Start in background now with English/German (not saved)
ccaps -stop        # Stop background process only (settings and auto-startup kept)
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

| Key Combination     | Action                          |
|---------------------|---------------------------------|
| `Caps Lock`         | Switch to next keyboard layout  |
| `Shift + Caps Lock` | Toggle Caps Lock on/off         |

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
- `enable` - Save settings, add to auto-startup and start now (all layouts)
- `enable -de` - Same, with specific layouts
- `disable` - Stop, remove from auto-startup and delete settings
- `start` - Start in background now, without touching auto-startup
- `stop` - Stop background process only (settings and auto-startup kept)
- `status` - Show current status and available language codes
- `help` - Show detailed help
- `menu` - Show menu again
- `exit` or `e` - Exit interactive menu

### 2. Switch Between English and German
```bash
ccaps -run -de
```

### 3. Switch Between Multiple Languages
```bash
ccaps -run -de -fr -es  # German ↔ French ↔ Spanish
```

### 4. Run in Background with English/German, Now and at Every Login
```bash
ccaps -enable -de
```

### 5. Run in Background Just for Now
```bash
ccaps -start       # saved settings (or all layouts)
ccaps -stop        # stop it again; nothing else changes
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
  ccaps -enable -de     # Run in background now and at every login (English/German)
  ccaps -start          # Run in background now, without auto-startup

Status: All systems operational ✓
```

## Configuration Persistence

CCaps saves your layout preferences when you use `-enable`:

- **Configuration file**: `ccaps-config.json` (stored in `%LOCALAPPDATA%\CCaps\`)
- **Typical location**: `C:\Users\<username>\AppData\Local\CCaps\ccaps-config.json`
- **Auto-restore**: Background process automatically loads saved preferences
- **JSON format**: Human-readable configuration file

Example configuration file:
```json
{
  "country_codes": ["de"],
  "version": "0.11.0"
}
```

### Configuration Management

- **Saving**: `ccaps -enable -de` saves the English/German preference
- **Auto-loading**: The background process loads saved preferences at login and on `ccaps -start`
- **Temporary codes**: `ccaps -start -de` uses English/German this time without saving it
- **Cleanup**: `ccaps -disable` removes the configuration file; `ccaps -stop` keeps it
- **Status check**: `ccaps -status` shows current configuration

## How It Works

CCaps uses Windows low-level keyboard hooks to intercept Caps Lock key presses and redirect them to layout switching functionality. The program:

1. Installs a system-wide keyboard hook
2. Intercepts Caps Lock key events
3. Cycles through selected keyboard layouts (filtered by country codes)
4. Asks the focused window to switch its layout, then checks that it actually did
5. If the window ignored the request, switches with Win+Space instead (see below)
6. Updates the Scroll Lock indicator from the layout the window actually has
7. Blocks the default Caps Lock behavior (unless Shift is held)
8. Saves and restores layout preferences automatically

### Applications That Ignore the Layout Request

Some applications don't handle the standard layout change request, so Caps Lock used to do nothing in them. CCaps now checks the layout after every switch. If it hasn't changed within 150 ms, CCaps presses **Win+Space** (the Windows layout switch shortcut) and re-reads the layout after each press until the target layout is reached.

- In such applications a switch takes about 150–200 ms instead of a few milliseconds
- Win+Space cycles through **all** installed layouts, so CCaps may press it several times to reach the next selected one; you may briefly see the intermediate layouts
- To avoid switching twice or triggering another shortcut, the fallback is skipped when the window is busy (not responding), when Shift, Ctrl, Alt or Win is held, when you press Caps Lock again, or when another window gets focus

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
# Run now and at every login with specific layouts (saved)
ccaps -enable -de         # English/German switching
ccaps -enable -de -fr     # German/French switching
ccaps -enable             # All layouts (default)

# Stop for now / start again; settings and auto-startup are kept
ccaps -stop
ccaps -start

# Remove CCaps from auto-startup and delete its settings
ccaps -disable
```

### Interactive Menu with Configuration
```bash
ccaps
# Choose from menu:
# enable -de    # This saves the preference, adds auto-startup and starts in background
# run -de       # This only runs temporarily without saving
# e             # Quick exit from interactive menu
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
- **Version**: 0.11.0
- **Windows APIs**: WinAPI (winuser, winreg, synchapi, fileapi)
- **Hook Type**: Low-level keyboard hook (WH_KEYBOARD_LL)
- **Registry**: Uses `HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`
- **Configuration**: JSON file in `%LOCALAPPDATA%\CCaps\`
- **Mutex**: Global mutex prevents multiple instances
- **Layout Detection**: Language ID extraction from HKL handles

## Known Limitations

- **Windows running as administrator**: when an elevated window (e.g. `cmd` or `regedit` started "as administrator") has focus, Windows doesn't pass keyboard input to CCaps, which runs with normal rights. In such windows Caps Lock works as a regular Caps Lock and doesn't switch the layout. Switch the layout with Win+Space there.
- **Terminals that control Scroll Lock (mintty: Cygwin, MSYS2, Git Bash)**: mintty resets the Scroll Lock LED itself, so the indicator doesn't reflect the layout while a mintty window has focus. Layout switching works normally.
- **Applications that ignore the layout request**: switched with Win+Space (see [Applications That Ignore the Layout Request](#applications-that-ignore-the-layout-request)), which is slightly slower.

## Troubleshooting

### Caps Lock toggles Caps Lock in some windows
The window is probably running as administrator; see [Known Limitations](#known-limitations).

### The Scroll Lock indicator doesn't match the layout
- In mintty-based terminals (Cygwin, MSYS2, Git Bash) this is expected; see [Known Limitations](#known-limitations)
- Elsewhere the indicator follows the actual layout within a quarter of a second, also after switching with Win+Space or the mouse

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

# Re-enable startup
ccaps -enable -de   # or your preferred layout codes
```

### Configuration not loading
- Check if configuration file exists: `ccaps -status`
- Restart background process: `ccaps -stop` then `ccaps -start`
- Delete and recreate: `ccaps -disable` then `ccaps -enable -de`

### Layout switching not working with specific codes
- Ensure the specified keyboard layouts are installed in Windows
- Check available codes with: `ccaps -status`
- Verify layouts in Settings → Time & Language → Language → Preferred languages

## Uninstall

```bash
# Stop the program and remove all traces
ccaps -disable

# Delete the executable file
del ccaps.exe

# Configuration file is automatically deleted by 'ccaps -disable'
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for the full version history.
