# Changelog

### v0.10.0
- 🔄 Renamed interactive menu commands: `exit` → `quit` (stop background process only), `quit`/`q` → `exit`/`e` (exit interactive menu)

### v0.9.0
- 🚀 Added automated publishing to crates.io via GitHub Actions workflow

### v0.8.3
- 🔧 Added GitHub Actions release workflow

### v0.8.2
- 🐛 Fixed spontaneous CapsLock LED activation during Windows startup caused by external injected events
- 🔧 Added dwExtraInfo marker (CCAPS_EXTRA_INFO) to distinguish CCaps's own SendInput calls from external ones
- 🔧 Added GetKeyboardState() synchronization before CapsLock state check at startup
- ✅ Added 7 unit tests for hook pass-through logic

### v0.8.1
- 🐛 Fixed sporadic CapsLock LED activation after Windows startup
- 🔧 Replaced blind double-toggle with state-aware CapsLock reset to avoid LED desync

### v0.8.0
- ✨ Changed hotkey for toggling Caps Lock from `Alt + Caps Lock` to `Shift + Caps Lock`
- 🔧 Improved modifier key handling for more intuitive Caps Lock toggle

### v0.7.2
- 🐛 Fixed sporadic CapsLock LED activation during Windows startup
- 🔧 Added WM_SYSKEYDOWN handling to block CapsLock events when Alt state is desynchronized
- 🔧 Added complete blocking of CapsLock key release events (WM_KEYUP/WM_SYSKEYUP)
- 🔧 Added CapsLock LED synchronization at program startup to fix LED/state desync
- 🔧 Improved SendInput handling by allowing injected events to pass through the hook

### v0.7.1
- 🐛 Fixed Scroll Lock indicator not being set correctly at autostart when non-English layout is active
- 🔧 Improved keyboard layout detection during startup using current thread layout instead of foreground window
- ✅ Added comprehensive unit tests for layout detection logic to prevent regression

### v0.7.0
- 🐛 Fixed sporadic Caps Lock LED activation during window switching and system startup
- 🔧 Improved Alt key state detection using real-time polling instead of event tracking
- 🔧 Removed dependency on Alt key state caching to prevent desynchronization
- ✨ Added confirmation prompts [y/n] for `-quit` and `-stop` commands to prevent accidental termination
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
