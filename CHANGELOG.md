# Changelog

### v0.11.0
- 🔍 `ccaps -run` prints diagnostics: one line per Caps Lock press with the time, layouts, outcome and its reason (e.g. switched with Win+Space, applied late, modifier key held), duration and the foreground application, plus a line whenever the layout changes without CCaps. The startup header is shorter: each layout is listed once, with the current one marked, followed by the keys and what the log lines mean
- 🔄 **Commands renamed** (systemctl-like: persistent state vs. the running process):
  - `-enable [-xx ...]` (was `-start`): save settings, add to auto-startup and start now
  - `-disable` (was `-stop`): stop, remove from auto-startup and delete settings
  - `-start [-xx ...]` (new): start the background process now; settings and auto-startup are not changed, codes given here are not saved
  - `-stop` (was `-quit`): stop the background process only; **no longer deletes anything**
  - `-quit` was removed and prints a hint; the interactive menu got the same commands
- 🐛 The background process no longer adds itself to auto-startup when the entry is missing (only `-enable` does)
- 🐛 `-start`/`-enable` wait until the background process is ready (up to 3 s) instead of reporting success at once: `ccaps -start` followed right away by `ccaps -stop` no longer leaves the process running, and a background process that exits right away (e.g. the saved settings name a layout that was removed) is reported with its exit code. Without country codes, `-start` also checks the saved codes first and names the unknown ones
- 🐛 The interactive menu no longer prints "Using all available layouts" for `start`/`enable` without codes (`start` uses the saved settings); each command reports the layouts it uses
- 🐛 `-start`/`-enable` no longer hang scripts that capture their output: the background process is created without inheriting the caller's handles (it used to keep the output pipe or redirected file open), and runs in the executable's folder instead of keeping the caller's current folder open
- ✅ Added unit tests for command parsing, country codes, `-status` recommendations and menu commands
- 🚀 Caps Lock now switches the layout in applications that ignore the layout request (`WM_INPUTLANGCHANGEREQUEST`): when the layout hasn't changed after 150 ms, CCaps presses Win+Space until the target layout is reached, checking the layout after every press. It never presses more than once per installed layout, and skips the fallback when it could switch twice or do something else (busy window, held modifier, a newer Caps Lock press, another window focused). Presses are 50 ms apart (the Windows language switcher sometimes lost a press sent right after the previous one), and a press that was lost is repeated once
- 🐛 Fixed Caps Lock not switching the layout in classic console windows (`cmd.exe` in conhost): their layout read as 0, so CCaps always picked the first layout and the Scroll Lock indicator blinked. The layout is now read from the console's input thread (via its IME window)
- 🐛 Fixed Caps Lock not switching in console programs that use window functions (e.g. PowerShell, REPLs started from a shortcut): their own thread holds a frozen layout, which hid the real one, so CCaps took a working switch for an ignored one and pressed Win+Space on top. In console windows the layout of conhost (the IME window's thread) is now read first
- 🐛 Fixed a stale layout in Windows 11 Notepad and UWP apps, where the focused control runs on another thread than the window: the current layout is now read from the focused control's thread, so the indicator and the next layout follow the actual one
- 🐛 The Scroll Lock indicator is left as it is when the layout can't be read, instead of being turned on
- ✅ Added unit tests for layout reading order and unreadable-layout handling
- 🔧 Layout switching moved from the keyboard hook to a worker thread: the hook only picks the target, so slow windows can no longer delay it past Windows' hook timeout. Rapid Caps Lock presses: the latest request wins
- 🐛 The Scroll Lock indicator now shows the layout the window actually has after a switch, not the one CCaps asked for (e.g. a window that ignored the request no longer makes it lie)
- 🔧 CCaps no longer calls `ActivateKeyboardLayout` on its own thread (it had no effect on other windows); the result of the layout request is now reported instead of ignored

### v0.10.2
- 🐛 Fixed Caps Lock not switching the layout in dialogs such as the Explorer "Save As" file name field: the layout request is now posted to the focused control of the foreground window (falling back to the foreground window itself)
- ✅ Added unit tests for the focused-control layout-request target

### v0.10.1
- 🐛 Fixed other applications (e.g. OpenGL apps on NVIDIA drivers) freezing on a layout switch: `WM_INPUTLANGCHANGEREQUEST` is now posted only to the foreground window instead of `HWND_BROADCAST`
- ✅ Added unit tests for the layout-request target
- 🐛 Fixed the Scroll Lock indicator drifting out of sync with the layout: it is now re-checked every 250 ms (a coalescable timer, so Windows can batch the wake-ups) against the foreground window's actual layout, so switching with Win+Space, focusing a window that has another layout, or a window that ignored the switch request no longer leaves it wrong
- 🐛 Caps Lock now switches to the layout after the foreground window's actual one, instead of following CCaps's own counter
- ✅ Added unit tests for next-layout selection and indicator sync decisions
- 🔧 Added CI workflow: formatting, clippy and tests on every push and pull request to `main`

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
