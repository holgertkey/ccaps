# CCaps Layout Switcher v0.5.0

A lightweight Windows keyboard layout switcher that repurposes the Caps Lock key for quick layout switching with country-specific filtering.

## Features

- **Caps Lock → Layout Switch**: Press Caps Lock to cycle through keyboard layouts
- **Country Code Filtering**: Choose specific layouts to switch between (e.g., English ↔ Russian)
- **Alt + Caps Lock → Caps Lock**: Hold Alt and press Caps Lock to toggle Caps Lock functionality
- **Visual Indicator**: Scroll Lock LED shows current layout (OFF = English, ON = Non-English)
- **Background Mode**: Runs silently in the background
- **Auto-startup**: Automatically starts with Windows
- **Low Resource Usage**: Minimal CPU and memory footprint
- **No Dependencies**: Single executable file

## Installation

1. Download the latest release from the [Releases](../../releases) page
2. Extract `ccaps.exe` to any folder (e.g., `C:\Program Files\CCaps\`)
3. Run the program using command line options

## Usage

### Command Line Options

```bash
# Basic commands
ccaps              # Show interactive menu
ccaps -run         # Run in foreground mode (all layouts)
ccaps -start       # Start in background + add to system startup
ccaps -stop        # Stop background process + remove from startup
ccaps -exit        # Stop background process only
ccaps -status      # Show status and available language codes
ccaps -help        # Show help information

# Country-specific switching
ccaps -run -ru     # English ↔ Russian switching
ccaps -run -ua     # English ↔ Ukrainian switching
ccaps -run -de     # English ↔ German switching
ccaps -run -de -fr # German ↔ French switching (no English)
```

### Country Codes

Use `ccaps -status` to see all available language codes for your system. Common codes include:

| Code | Language | Code | Language | Code | Language |
|------|----------|------|----------|------|----------|
| `us` | English (US) | `ru` | Russian | `ua` | Ukrainian |
| `gb` | English (UK) | `de` | German | `fr` | French |
| `es` | Spanish | `it` | Italian | `pl` | Polish |
| `pt` | Portuguese | `nl` | Dutch | `cz` | Czech |
| `jp` | Japanese | `kr` | Korean | `cn` | Chinese |

### Key Bindings

| Key Combination | Action |
|-----------------|--------|
| `Caps Lock` | Switch to next keyboard layout |
| `Alt + Caps Lock` | Toggle Caps Lock on/off |

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

### 2. Switch Between English and Russian
```bash
ccaps -run -ru
```

### 3. Switch Between Multiple Languages
```bash
ccaps -run -de -fr -es  # German ↔ French ↔ Spanish
```

### 4. Start in Background with Auto-startup
```bash
ccaps -start
```

### 5. Check Available Languages
```bash
ccaps -status
```
Output example:
```
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
  ccaps -run -ru     # Switch between English and Russian
  ccaps -run -ua     # Switch between English and Ukrainian
  ccaps -run -de -fr # Switch between German and French
```

## How It Works

CCaps uses Windows low-level keyboard hooks to intercept Caps Lock key presses and redirect them to layout switching functionality. The program:

1. Installs a system-wide keyboard hook
2. Intercepts Caps Lock key events
3. Cycles through selected keyboard layouts (filtered by country codes)
4. Updates the Scroll Lock indicator to show the current layout
5. Blocks the default Caps Lock behavior (unless Alt is held)

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

### Background Process Management
```bash
# Start with specific layouts and auto-startup
ccaps -start
# The background process will use all layouts by default

# For country-specific background switching, use:
ccaps -run -ru     # Then minimize or run in background manually
```

### Registry Integration
The program stores startup configuration in:
```
HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
Key: "CCaps Layout Switcher"
```

### Status Monitoring
```bash
ccaps -status
```
Shows:
- Background process status
- Auto-startup configuration
- All available keyboard layouts with country codes
- Current active layout
- Usage examples

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

### Build Configuration

The project is optimized for size in release mode:

```toml
[profile.release]
opt-level = "s"    # Optimize for size
lto = true         # Link Time Optimization  
codegen-units = 1  # Better optimization
panic = "abort"    # Smaller binary size
strip = true       # Remove debug info
```

## Technical Details

- **Language**: Rust
- **Version**: 0.5.0
- **Windows APIs**: WinAPI (winuser, winreg, synchapi)
- **Hook Type**: Low-level keyboard hook (WH_KEYBOARD_LL)
- **Registry**: Uses `HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`
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
ccaps -start
```

### Only one layout switching
If you specify a country code but only have English installed, the program will switch between the current layout and itself. Install additional keyboard layouts in Windows Settings.

### Layout switching not working with specific codes
- Ensure the specified keyboard layouts are installed in Windows
- Check available codes with: `ccaps -status`
- Verify layouts in Settings → Time & Language → Language → Preferred languages

### Scroll Lock indicator not updating
- Some keyboards don't have Scroll Lock LEDs
- Try using software that shows Scroll Lock status
- The functionality works even without visible indicator

## Migration from v0.4.0

Version 0.5.0 is backward compatible with v0.4.0 commands:

- `ccaps -run` works the same (cycles through all layouts)
- `ccaps -start`, `ccaps -stop`, `ccaps -status` unchanged
- New: Country code filtering with `-ru`, `-ua`, etc.

## Uninstall

```bash
# Stop the program and remove from startup
ccaps -stop

# Delete the executable file
del ccaps.exe
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Changelog

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