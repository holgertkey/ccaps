# CCaps Layout Switcher

A lightweight Windows keyboard layout switcher that repurposes the Caps Lock key for quick layout switching.

## Features

- **Caps Lock → Layout Switch**: Press Caps Lock to cycle through keyboard layouts
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
ccaps          # Run in foreground mode
ccaps -start   # Start in background and add to system startup
ccaps -stop    # Stop background process and remove from startup
ccaps -exit    # Stop background process only (keep startup entry)
ccaps -status  # Show current status
ccaps -help    # Show help information
```

### Key Bindings

| Key Combination | Action |
|-----------------|--------|
| `Caps Lock` | Switch to next keyboard layout |
| `Alt + Caps Lock` | Toggle Caps Lock on/off |

### Visual Indicator

The Scroll Lock LED on your keyboard serves as a layout indicator:
- **OFF** (🔴) = English layout active
- **ON** (🟢) = Non-English layout active

## Quick Start

1. **Start with interactive menu**:
   ```bash
   ccaps
   ```

2. **Run in foreground mode**:
   ```bash
   ccaps -run
   ```

3. **Start in background + add to startup**:
   ```bash
   ccaps -start
   ```

4. **Stop + remove from startup**:
   ```bash
   ccaps -stop
   ```

## Status Output Example

```
CCaps Layout Switcher Status:
═══════════════════════════════
Background process: RUNNING ✓
Auto-startup:       ENABLED ✓
Startup command:    "C:\Tools\ccaps.exe" --background

Status: All systems operational ✓
```

## How It Works

CCaps uses Windows low-level keyboard hooks to intercept Caps Lock key presses and redirect them to layout switching functionality. The program:

1. Installs a system-wide keyboard hook
2. Intercepts Caps Lock key events
3. Cycles through available keyboard layouts
4. Updates the Scroll Lock indicator to show the current layout
5. Blocks the default Caps Lock behavior (unless Alt is held)

## Supported Languages

The layout detection works with all Windows keyboard layouts. English layouts are automatically detected based on language IDs:

- English (US, UK, Australia, Canada, etc.)
- All other languages (Russian, Ukrainian, German, French, Spanish, etc.)

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
- **Windows APIs**: WinAPI (winuser, winreg, synchapi)
- **Hook Type**: Low-level keyboard hook (WH_KEYBOARD_LL)
- **Registry**: Uses `HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`
- **Mutex**: Global mutex prevents multiple instances

## Troubleshooting

### Program doesn't start with Windows
```bash
# Check status
ccaps -status

# Restart and re-enable startup
ccaps -stop
ccaps -start
```

### Caps Lock not working as expected
- Make sure only one instance is running
- Try restarting the program: `ccaps -exit` then `ccaps -start`
- Check if other keyboard software is interfering

### Layout switching not working
- Ensure you have multiple keyboard layouts installed in Windows
- Verify layouts in Settings → Time & Language → Language → Preferred languages

### Scroll Lock indicator not updating
- Some keyboards don't have Scroll Lock LEDs
- Try using software that shows Scroll Lock status
- The functionality works even without visible indicator

## Uninstall

```bash
# Stop the program and remove from startup
ccaps -stop

# Delete the executable file
del ccaps.exe
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.