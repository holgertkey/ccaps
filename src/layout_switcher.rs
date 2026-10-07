// Layout switching with verification, and a Win+Space fallback for windows that ignore
// WM_INPUTLANGCHANGEREQUEST.
//
// The decision logic works through the SwitchOps trait, so it can be tested against a
// fake window without touching WinAPI.
//
// Switching runs on a worker thread, never in the keyboard hook: Windows silently removes
// a low-level hook that doesn't return quickly (LowLevelHooksTimeout), and verifying a
// switch means waiting. The hook only sends a request; when a switch is done, the worker
// posts WM_SWITCH_DONE to the main thread with the layout it read afterwards.

use crate::console_log;
use crate::keyboard_hook::CCAPS_EXTRA_INFO;
use crate::layout_manager::{self, LayoutInfo};
use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use winapi::shared::minwindef::{HKL, LPARAM, WPARAM};
use winapi::um::winuser::{
    GetAsyncKeyState, GetForegroundWindow, PostThreadMessageW, SendInput, INPUT, INPUT_KEYBOARD,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    VK_SPACE, WM_APP,
};

// Posted to the main thread when a switch is finished: wParam = request generation,
// lParam = layout read afterwards (0 if it can't be read)
pub const WM_SWITCH_DONE: u32 = WM_APP + 1;

struct SwitchRequest {
    target: usize,
    generation: u64,
}

// Generation of the latest request; a worker busy with an older one drops it
static LATEST_GENERATION: AtomicU64 = AtomicU64::new(0);
static REQUESTS: Mutex<Option<Sender<SwitchRequest>>> = Mutex::new(None);

// Starts the worker thread. Results go to `main_thread` as WM_SWITCH_DONE; with console
// diagnostics enabled (foreground mode) every switch is also printed.
pub fn start(main_thread: u32) {
    let (sender, receiver) = mpsc::channel();
    if let Ok(mut requests) = REQUESTS.lock() {
        *requests = Some(sender);
    }
    std::thread::spawn(move || run_worker(receiver, main_thread));
}

// Stops the worker thread once it finishes the current switch
pub fn stop() {
    if let Ok(mut requests) = REQUESTS.lock() {
        requests.take();
    }
}

// Asks the worker to switch the foreground window to `target`. Returns the request's
// generation, which comes back with WM_SWITCH_DONE, or None if the worker isn't running.
pub fn request(target: usize) -> Option<u64> {
    let generation = LATEST_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let requests = REQUESTS.lock().ok()?;
    requests
        .as_ref()?
        .send(SwitchRequest { target, generation })
        .ok()?;
    Some(generation)
}

fn run_worker(receiver: Receiver<SwitchRequest>, main_thread: u32) {
    let ops = SystemOps {
        fallback_enabled: true,
    };
    // Requests dropped for a newer one since the last reported switch (diagnostics)
    let mut skipped = 0;
    // Ends when stop() drops the sender
    while let Ok(first) = receiver.recv() {
        // Rapid presses: only the latest request matters
        let (request, dropped) = latest_request(first, &receiver);
        skipped += dropped;
        let generation = request.generation;
        let superseded = || LATEST_GENERATION.load(Ordering::SeqCst) != generation;

        let started = Instant::now();
        let from = ops.current_layout();
        let app = console_log::enabled().then(layout_manager::foreground_app);

        let report = perform_switch(&ops, request.target, &superseded);
        if report.outcome == SwitchOutcome::Superseded {
            // The newer request is already queued and will report its own result
            skipped += 1;
            continue;
        }
        let layout = ops.current_layout().unwrap_or(0);
        if let Some(app) = app {
            console_log::log(&describe_switch(
                &report,
                &from.map_or_else(|| "?".to_string(), layout_code),
                &layout_code(request.target),
                started.elapsed().as_millis(),
                &app,
                skipped,
            ));
        }
        skipped = 0;
        unsafe {
            PostThreadMessageW(
                main_thread,
                WM_SWITCH_DONE,
                generation as WPARAM,
                layout as LPARAM,
            );
        }
    }
}

// The last of `first` and the requests already waiting in the channel, and how many
// older requests were dropped
fn latest_request(
    first: SwitchRequest,
    receiver: &Receiver<SwitchRequest>,
) -> (SwitchRequest, usize) {
    receiver
        .try_iter()
        .fold((first, 0), |(_, dropped), newer| (newer, dropped + 1))
}

pub fn layout_code(hkl: usize) -> String {
    LayoutInfo::new(hkl as HKL).short_code
}

// How long the window gets to apply WM_INPUTLANGCHANGEREQUEST before CCaps falls back
// to Win+Space (measured: 5-26 ms)
pub const VERIFY_TIMEOUT_MS: u32 = 150;
// How long to wait for the layout to change after one Win+Space (measured: 5-11 ms)
pub const PRESS_TIMEOUT_MS: u32 = 100;
// Pause after a press changed the layout, before the next press. Presses sent right one
// after another were sometimes lost by the Windows language switcher (about 1 in 20
// three-press switches, always the third press).
pub const PRESS_SETTLE_MS: u32 = 50;
// Extra wait when a press had no visible effect, before it counts as lost
pub const LOST_PRESS_WAIT_MS: u32 = 200;
// Lost presses repeated per switch
pub const MAX_PRESS_RETRIES: usize = 1;
// Interval between layout reads while waiting
pub const POLL_INTERVAL_MS: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchOutcome {
    // The window applied the request
    Switched,
    // The target was reached with Win+Space
    SwitchedByFallback,
    // Unknown whether the target was reached: the layout can't be read, or the window
    // doesn't respond (the request may still be applied later)
    Unverified,
    // The target was not reached
    Failed,
    // A newer request arrived; this one was dropped
    Superseded,
}

// Everything the switch needs from the system. Layouts are HKL values as usize.
pub trait SwitchOps {
    // Identity of the foreground window, to notice when the user moves elsewhere
    fn foreground(&self) -> usize;
    // Layout of the foreground window's input thread, None if it can't be read
    fn current_layout(&self) -> Option<usize>;
    // Posts WM_INPUTLANGCHANGEREQUEST; false if the window rejected it (e.g. an elevated
    // window, where injected keys would be blocked as well)
    fn post_request(&self, target: usize) -> bool;
    fn is_installed(&self, layout: usize) -> bool;
    fn installed_layout_count(&self) -> usize;
    // Whether the user holds Shift, Ctrl, Alt or Win: Win+Space would become another
    // shortcut
    fn modifiers_held(&self) -> bool;
    // Whether the foreground window processes messages. A busy window still holds the
    // posted request and would apply it on top of the fallback.
    fn target_responsive(&self) -> bool;
    // Injects Win+Space; false if the input was not inserted
    fn press_win_space(&self) -> bool;
    fn sleep_ms(&self, ms: u32);
}

// How long the foreground window gets to answer before it counts as busy
const RESPONSIVE_TIMEOUT_MS: u32 = 100;

// SwitchOps backed by WinAPI
pub struct SystemOps {
    // Whether Win+Space may be pressed when the window ignores the request
    pub fallback_enabled: bool,
}

impl SwitchOps for SystemOps {
    fn foreground(&self) -> usize {
        unsafe { GetForegroundWindow() as usize }
    }

    fn current_layout(&self) -> Option<usize> {
        layout_manager::current_layout_hkl()
    }

    fn post_request(&self, target: usize) -> bool {
        layout_manager::post_layout_request(target)
    }

    fn is_installed(&self, layout: usize) -> bool {
        layout_manager::installed_layouts().contains(&layout)
    }

    fn installed_layout_count(&self) -> usize {
        layout_manager::installed_layouts().len()
    }

    fn modifiers_held(&self) -> bool {
        [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|&vk| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0)
    }

    fn target_responsive(&self) -> bool {
        layout_manager::foreground_responsive(RESPONSIVE_TIMEOUT_MS)
    }

    fn press_win_space(&self) -> bool {
        if !self.fallback_enabled {
            return false;
        }
        unsafe {
            let mut inputs = win_space_inputs();
            let sent = SendInput(
                inputs.len() as u32,
                inputs.as_mut_ptr(),
                mem::size_of::<INPUT>() as i32,
            );
            if (sent as usize) < inputs.len() {
                // Never leave Win held down
                let mut release = win_space_release_inputs();
                SendInput(
                    release.len() as u32,
                    release.as_mut_ptr(),
                    mem::size_of::<INPUT>() as i32,
                );
                return false;
            }
            true
        }
    }

    fn sleep_ms(&self, ms: u32) {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }
}

// What happened during a switch, for the outcome and the diagnostics in foreground mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchReport {
    pub outcome: SwitchOutcome,
    // Why the switch didn't go the normal way; None when there is nothing to explain
    pub reason: Option<&'static str>,
    // Win+Space presses made
    pub presses: usize,
    // The window applied the request, but only after the verification timeout
    pub late: bool,
    // Win+Space presses that were lost and repeated (included in `presses`)
    pub retries: usize,
}

impl SwitchReport {
    fn new(outcome: SwitchOutcome, reason: Option<&'static str>, presses: usize) -> Self {
        SwitchReport {
            outcome,
            reason,
            presses,
            late: false,
            retries: 0,
        }
    }
}

// Lets tests compare a report with the expected outcome directly
impl PartialEq<SwitchOutcome> for SwitchReport {
    fn eq(&self, outcome: &SwitchOutcome) -> bool {
        self.outcome == *outcome
    }
}

pub const REASON_REJECTED: &str = "request rejected (elevated window?)";
pub const REASON_UNREADABLE: &str = "layout can't be read";
pub const REASON_SUPERSEDED: &str = "a newer Caps Lock press";
pub const REASON_MODIFIER: &str = "a modifier key is held, no Win+Space";
pub const REASON_NOT_INSTALLED: &str = "target layout is not installed";
pub const REASON_NOT_RESPONDING: &str = "window not responding, no Win+Space";
pub const REASON_FULL_CYCLE: &str = "not reached after a full Win+Space cycle";
pub const REASON_FOCUS_MOVED: &str = "another window got focus";
pub const REASON_PRESS_FAILED: &str = "Win+Space could not be sent";
pub const REASON_PRESS_NO_EFFECT: &str = "Win+Space had no effect";

// Switches the foreground window to `target`: posts the request, verifies it, and if the
// window ignored it, presses Win+Space until the target is reached. `superseded` tells
// whether a newer request is waiting.
pub fn perform_switch(
    ops: &impl SwitchOps,
    target: usize,
    superseded: &dyn Fn() -> bool,
) -> SwitchReport {
    use SwitchOutcome::*;
    let done = |outcome, reason| SwitchReport::new(outcome, Some(reason), 0);

    let window = ops.foreground();
    if !ops.post_request(target) {
        return done(Failed, REASON_REJECTED);
    }

    let Some(current) = poll_layout(ops, VERIFY_TIMEOUT_MS, |layout| layout == target) else {
        return done(Unverified, REASON_UNREADABLE);
    };
    if current == target {
        return SwitchReport::new(Switched, None, 0);
    }

    // The window ignored the request (or hasn't applied it yet)
    if superseded() {
        return done(Superseded, REASON_SUPERSEDED);
    }
    if ops.modifiers_held() {
        return done(Failed, REASON_MODIFIER);
    }
    if !ops.is_installed(target) {
        return done(Failed, REASON_NOT_INSTALLED);
    }
    if !ops.target_responsive() {
        return done(Unverified, REASON_NOT_RESPONDING);
    }

    // Win+Space cycles through every installed layout, in an order CCaps can't know in
    // advance: press and re-read until the target, at most one full cycle. The first
    // read happens before any press, so a request applied late counts as Switched.
    // `steps` counts presses that changed the layout (the cycle limit); `presses` also
    // counts lost ones that were repeated.
    let max_steps = ops.installed_layout_count();
    let mut steps = 0;
    let mut presses = 0;
    let mut retries = 0;
    loop {
        let stop = |outcome, reason| SwitchReport {
            retries,
            ..SwitchReport::new(outcome, Some(reason), presses)
        };
        let Some(current) = ops.current_layout() else {
            return stop(Unverified, REASON_UNREADABLE);
        };
        match fallback_step(current, target, steps, max_steps) {
            FallbackStep::Done if presses == 0 => {
                return SwitchReport {
                    late: true,
                    ..SwitchReport::new(Switched, None, 0)
                }
            }
            FallbackStep::Done => {
                return SwitchReport {
                    retries,
                    ..SwitchReport::new(SwitchedByFallback, None, presses)
                }
            }
            FallbackStep::GiveUp => return stop(Failed, REASON_FULL_CYCLE),
            FallbackStep::PressAgain => {}
        }
        if presses > 0 {
            // Give the language switcher time after the previous press; done before
            // the checks below, so a newer request or a modifier pressed meanwhile counts
            ops.sleep_ms(PRESS_SETTLE_MS);
        }
        if superseded() {
            return stop(Superseded, REASON_SUPERSEDED);
        }
        if ops.foreground() != window {
            return stop(Failed, REASON_FOCUS_MOVED);
        }
        if ops.modifiers_held() {
            return stop(Failed, REASON_MODIFIER);
        }
        if !ops.press_win_space() {
            return stop(Failed, REASON_PRESS_FAILED);
        }
        presses += 1;
        match wait_for_press(ops, current) {
            PressResult::Changed => steps += 1,
            // Lost: no change even after the extra wait, so pressing again can't
            // overshoot because of it. Repeat it, but only so many times.
            PressResult::Lost if retries < MAX_PRESS_RETRIES => retries += 1,
            PressResult::Lost => {
                return SwitchReport {
                    retries,
                    ..SwitchReport::new(Unverified, Some(REASON_PRESS_NO_EFFECT), presses)
                }
            }
            PressResult::Unreadable => {
                return SwitchReport {
                    retries,
                    ..SwitchReport::new(Unverified, Some(REASON_UNREADABLE), presses)
                }
            }
        }
    }
}

enum PressResult {
    Changed,
    Lost,
    Unreadable,
}

// Waits for the layout to change from `before` after a Win+Space: PRESS_TIMEOUT_MS as
// measured, then LOST_PRESS_WAIT_MS more in case the press is only slow
fn wait_for_press(ops: &impl SwitchOps, before: usize) -> PressResult {
    for timeout in [PRESS_TIMEOUT_MS, LOST_PRESS_WAIT_MS] {
        match poll_layout(ops, timeout, |layout| layout != before) {
            Some(layout) if layout != before => return PressResult::Changed,
            Some(_) => {}
            None => return PressResult::Unreadable,
        }
    }
    PressResult::Lost
}

// One diagnostics line for a finished switch, e.g.
// "us → ru  SwitchedByFallback: 2× Win+Space, 231 ms · Code.exe (Chrome_WidgetWin_1)"
pub fn describe_switch(
    report: &SwitchReport,
    from: &str,
    to: &str,
    elapsed_ms: u128,
    app: &str,
    superseded: usize,
) -> String {
    let mut details = Vec::new();
    if report.late {
        details.push(format!(
            "applied after the {} ms check, no Win+Space",
            VERIFY_TIMEOUT_MS
        ));
    }
    if report.presses > 0 {
        details.push(format!("{}× Win+Space", report.presses));
    }
    if report.retries > 0 {
        details.push(format!("{} lost and repeated", report.retries));
    }
    if let Some(reason) = report.reason {
        details.push(reason.to_string());
    }
    details.push(format!("{} ms", elapsed_ms));

    let outcome = if report.late {
        "Switched late".to_string()
    } else {
        format!("{:?}", report.outcome)
    };
    let mut line = format!(
        "{} → {}  {}: {} · {}",
        from,
        to,
        outcome,
        details.join(", "),
        app
    );
    if superseded > 0 {
        line.push_str(&format!(" (+{} earlier press(es) skipped)", superseded));
    }
    line
}

// Reads the layout every POLL_INTERVAL_MS until `done` accepts it or `timeout_ms`
// passes. Returns the last layout read, None as soon as it can't be read.
fn poll_layout(
    ops: &impl SwitchOps,
    timeout_ms: u32,
    done: impl Fn(usize) -> bool,
) -> Option<usize> {
    let mut waited = 0;
    loop {
        let layout = ops.current_layout()?;
        if done(layout) || waited >= timeout_ms {
            return Some(layout);
        }
        ops.sleep_ms(POLL_INTERVAL_MS);
        waited += POLL_INTERVAL_MS;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackStep {
    Done,
    PressAgain,
    GiveUp,
}

// Next step of the Win+Space loop. Checking the target before every press (including
// the first) guards against a double switch when the request was applied after all.
pub fn fallback_step(
    current: usize,
    target: usize,
    presses_done: usize,
    max_presses: usize,
) -> FallbackStep {
    if current == target {
        FallbackStep::Done
    } else if presses_done >= max_presses {
        FallbackStep::GiveUp
    } else {
        FallbackStep::PressAgain
    }
}

// Win down, Space down, Space up, Win up, marked as CCaps's own input. They must be sent
// in one SendInput call: it inserts them as a block, so the user's typing can't land in
// between and turn Win into another shortcut (Win+L, Win+E, ...).
pub fn win_space_inputs() -> [INPUT; 4] {
    [
        key_input(VK_LWIN, false),
        key_input(VK_SPACE, false),
        key_input(VK_SPACE, true),
        key_input(VK_LWIN, true),
    ]
}

// Releases for both keys, sent when SendInput inserted only part of win_space_inputs(),
// so Win doesn't stay held down or open the Start menu
pub fn win_space_release_inputs() -> [INPUT; 2] {
    [key_input(VK_SPACE, true), key_input(VK_LWIN, true)]
}

fn key_input(vk: i32, key_up: bool) -> INPUT {
    unsafe {
        let mut input: INPUT = mem::zeroed();
        input.type_ = INPUT_KEYBOARD;
        let ki = input.u.ki_mut();
        ki.wVk = vk as u16;
        ki.dwFlags = if key_up { KEYEVENTF_KEYUP } else { 0 }
            | if vk == VK_LWIN {
                KEYEVENTF_EXTENDEDKEY
            } else {
                0
            };
        ki.dwExtraInfo = CCAPS_EXTRA_INFO;
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const RU: usize = 0x0419_0419;
    const US: usize = 0x0409_0409;
    const UA: usize = 0xF0A8_0422;
    const DE: usize = 0x0407_0407;
    const FR: usize = 0x040C_040C;

    // Win+Space order measured on a machine with four layouts
    const CYCLE: [usize; 4] = [RU, US, UA, DE];

    // A foreground window with configurable behavior
    struct FakeWindow {
        layout: Cell<usize>,
        deaf: bool,
        rejects_request: bool,
        unreadable: bool,
        responsive: bool,
        // Holds the request until the window responds again
        busy: bool,
        delayed_request: Cell<Option<usize>>,
        ignores_win_space: bool,
        // Win+Space presses (1-based) that are lost
        lost_presses: Vec<usize>,
        modifiers: bool,
        // Foreground changes once this many presses were made
        switch_window_after: Option<usize>,
        presses: Cell<usize>,
    }

    impl FakeWindow {
        fn new(layout: usize) -> Self {
            FakeWindow {
                layout: Cell::new(layout),
                deaf: false,
                rejects_request: false,
                unreadable: false,
                responsive: true,
                busy: false,
                delayed_request: Cell::new(None),
                ignores_win_space: false,
                lost_presses: Vec::new(),
                modifiers: false,
                switch_window_after: None,
                presses: Cell::new(0),
            }
        }

        fn deaf(layout: usize) -> Self {
            FakeWindow {
                deaf: true,
                ..FakeWindow::new(layout)
            }
        }
    }

    impl SwitchOps for FakeWindow {
        fn foreground(&self) -> usize {
            match self.switch_window_after {
                Some(n) if self.presses.get() >= n => 2,
                _ => 1,
            }
        }

        fn current_layout(&self) -> Option<usize> {
            (!self.unreadable).then(|| self.layout.get())
        }

        fn post_request(&self, target: usize) -> bool {
            if self.rejects_request {
                return false;
            }
            if self.busy {
                self.delayed_request.set(Some(target));
            } else if !self.deaf {
                self.layout.set(target);
            }
            true
        }

        fn is_installed(&self, layout: usize) -> bool {
            CYCLE.contains(&layout)
        }

        fn installed_layout_count(&self) -> usize {
            CYCLE.len()
        }

        fn modifiers_held(&self) -> bool {
            self.modifiers
        }

        fn target_responsive(&self) -> bool {
            if self.responsive {
                // The window pumped its queue: a delayed request is applied now
                if let Some(target) = self.delayed_request.take() {
                    self.layout.set(target);
                }
            }
            self.responsive
        }

        fn press_win_space(&self) -> bool {
            self.presses.set(self.presses.get() + 1);
            if !self.ignores_win_space && !self.lost_presses.contains(&self.presses.get()) {
                let index = CYCLE.iter().position(|&l| l == self.layout.get()).unwrap();
                self.layout.set(CYCLE[(index + 1) % CYCLE.len()]);
            }
            true
        }

        fn sleep_ms(&self, _ms: u32) {}
    }

    fn never() -> bool {
        false
    }

    #[test]
    fn test_normal_window_switches_without_fallback() {
        let window = FakeWindow::new(US);
        assert_eq!(perform_switch(&window, RU, &never), SwitchOutcome::Switched);
        assert_eq!(window.layout.get(), RU);
        assert_eq!(window.presses.get(), 0);
    }

    #[test]
    fn test_deaf_window_reaches_adjacent_target_with_one_press() {
        let window = FakeWindow::deaf(RU);
        assert_eq!(
            perform_switch(&window, US, &never),
            SwitchOutcome::SwitchedByFallback
        );
        assert_eq!(window.layout.get(), US);
        assert_eq!(window.presses.get(), 1);
    }

    #[test]
    fn test_deaf_window_cycles_through_unselected_layouts() {
        // Two layouts selected in CCaps (us, ru), four installed: from us, Win+Space
        // passes ua and de before ru, so the cap must count installed layouts
        let window = FakeWindow::deaf(US);
        assert_eq!(
            perform_switch(&window, RU, &never),
            SwitchOutcome::SwitchedByFallback
        );
        assert_eq!(window.layout.get(), RU);
        assert_eq!(window.presses.get(), 3);
    }

    #[test]
    fn test_rejected_request_does_not_fall_back() {
        // e.g. an elevated window: injected keys would be blocked too
        let window = FakeWindow {
            rejects_request: true,
            ..FakeWindow::new(US)
        };
        assert_eq!(perform_switch(&window, RU, &never), SwitchOutcome::Failed);
        assert_eq!(window.presses.get(), 0);
    }

    #[test]
    fn test_unreadable_layout_does_not_fall_back() {
        let window = FakeWindow {
            unreadable: true,
            ..FakeWindow::deaf(US)
        };
        assert_eq!(
            perform_switch(&window, RU, &never),
            SwitchOutcome::Unverified
        );
        assert_eq!(window.presses.get(), 0);
    }

    #[test]
    fn test_busy_window_does_not_fall_back() {
        // The request is still queued: pressing Win+Space now would switch twice
        let window = FakeWindow {
            busy: true,
            responsive: false,
            ..FakeWindow::new(US)
        };
        assert_eq!(
            perform_switch(&window, RU, &never),
            SwitchOutcome::Unverified
        );
        assert_eq!(window.presses.get(), 0);
    }

    #[test]
    fn test_request_applied_late_is_not_switched_again() {
        // The window was busy during verification, then applied the request when it
        // responded: the target is already reached, no Win+Space
        let window = FakeWindow {
            busy: true,
            ..FakeWindow::new(US)
        };
        assert_eq!(perform_switch(&window, RU, &never), SwitchOutcome::Switched);
        assert_eq!(window.layout.get(), RU);
        assert_eq!(window.presses.get(), 0);
    }

    #[test]
    fn test_held_modifiers_prevent_fallback() {
        // Shift+Win+Space or Ctrl+Win+Space would do something else
        let window = FakeWindow {
            modifiers: true,
            ..FakeWindow::deaf(US)
        };
        assert_eq!(perform_switch(&window, RU, &never), SwitchOutcome::Failed);
        assert_eq!(window.presses.get(), 0);
    }

    #[test]
    fn test_target_not_installed_does_not_fall_back() {
        let window = FakeWindow::deaf(US);
        assert_eq!(perform_switch(&window, FR, &never), SwitchOutcome::Failed);
        assert_eq!(window.presses.get(), 0);
    }

    #[test]
    fn test_fallback_stops_when_a_press_has_no_effect() {
        let window = FakeWindow {
            ignores_win_space: true,
            ..FakeWindow::deaf(US)
        };
        assert_eq!(
            perform_switch(&window, RU, &never),
            SwitchOutcome::Unverified
        );
        // The first press and its one repetition had no effect, so CCaps stops there
        assert_eq!(window.presses.get(), 1 + MAX_PRESS_RETRIES);
    }

    #[test]
    fn test_fallback_never_exceeds_installed_layout_count() {
        // The target is installed but Win+Space never reaches it (e.g. the cycle skips
        // it): at most one press per installed layout
        let window = FakeWindow::deaf(US);
        let unreachable = 0x1234_1234;
        let fake_installed = FakeInstalled(&window, unreachable);
        assert_eq!(
            perform_switch(&fake_installed, unreachable, &never),
            SwitchOutcome::Failed
        );
        assert_eq!(window.presses.get(), CYCLE.len());
    }

    // Wraps a FakeWindow and claims one extra layout is installed
    struct FakeInstalled<'a>(&'a FakeWindow, usize);

    impl SwitchOps for FakeInstalled<'_> {
        fn foreground(&self) -> usize {
            self.0.foreground()
        }
        fn current_layout(&self) -> Option<usize> {
            self.0.current_layout()
        }
        fn post_request(&self, target: usize) -> bool {
            self.0.post_request(target)
        }
        fn is_installed(&self, layout: usize) -> bool {
            layout == self.1 || self.0.is_installed(layout)
        }
        fn installed_layout_count(&self) -> usize {
            self.0.installed_layout_count()
        }
        fn modifiers_held(&self) -> bool {
            self.0.modifiers_held()
        }
        fn target_responsive(&self) -> bool {
            self.0.target_responsive()
        }
        fn press_win_space(&self) -> bool {
            self.0.press_win_space()
        }
        fn sleep_ms(&self, ms: u32) {
            self.0.sleep_ms(ms)
        }
    }

    #[test]
    fn test_lost_press_is_repeated() {
        // Regression: the Windows language switcher sometimes lost the third of three
        // quick presses, leaving the window on the wrong layout (de instead of ru)
        let window = FakeWindow {
            lost_presses: vec![3],
            ..FakeWindow::deaf(US)
        };
        let report = perform_switch(&window, RU, &never);
        assert_eq!(report.outcome, SwitchOutcome::SwitchedByFallback);
        assert_eq!(window.layout.get(), RU);
        assert_eq!(report.presses, 4);
        assert_eq!(report.retries, 1);
    }

    #[test]
    fn test_only_one_lost_press_is_repeated() {
        let window = FakeWindow {
            lost_presses: vec![1, 2],
            ..FakeWindow::deaf(US)
        };
        let report = perform_switch(&window, RU, &never);
        assert_eq!(report.outcome, SwitchOutcome::Unverified);
        assert_eq!(report.reason, Some(REASON_PRESS_NO_EFFECT));
        assert_eq!(report.presses, 2);
    }

    #[test]
    fn test_lost_press_does_not_shorten_the_cycle_limit() {
        // A repeated press doesn't count against "one press per installed layout":
        // the target three steps away is still reached
        let window = FakeWindow {
            lost_presses: vec![1],
            ..FakeWindow::deaf(US)
        };
        assert_eq!(
            perform_switch(&window, RU, &never),
            SwitchOutcome::SwitchedByFallback
        );
        assert_eq!(window.presses.get(), 4);
    }

    #[test]
    fn test_describe_mentions_repeated_press() {
        let window = FakeWindow {
            lost_presses: vec![3],
            ..FakeWindow::deaf(US)
        };
        let report = perform_switch(&window, RU, &never);
        assert_eq!(
            describe_switch(&report, "us", "ru", 520, "app.exe (Cls)", 0),
            "us → ru  SwitchedByFallback: 4× Win+Space, 1 lost and repeated, 520 ms · app.exe (Cls)"
        );
    }

    #[test]
    fn test_newer_request_stops_the_fallback() {
        let window = FakeWindow::deaf(US);
        let superseded = || window.presses.get() >= 1;
        assert_eq!(
            perform_switch(&window, RU, &superseded),
            SwitchOutcome::Superseded
        );
        assert_eq!(window.presses.get(), 1);
    }

    #[test]
    fn test_newer_request_before_fallback_skips_it() {
        let window = FakeWindow::deaf(US);
        let always = || true;
        assert_eq!(
            perform_switch(&window, RU, &always),
            SwitchOutcome::Superseded
        );
        assert_eq!(window.presses.get(), 0);
    }

    #[test]
    fn test_fallback_stops_when_foreground_window_changes() {
        let window = FakeWindow {
            switch_window_after: Some(1),
            ..FakeWindow::deaf(US)
        };
        assert_eq!(perform_switch(&window, RU, &never), SwitchOutcome::Failed);
        assert_eq!(window.presses.get(), 1);
    }

    #[test]
    fn test_latest_request_wins() {
        let (sender, receiver) = mpsc::channel();
        for generation in 2..=4 {
            sender
                .send(SwitchRequest {
                    target: generation as usize,
                    generation,
                })
                .unwrap();
        }
        let first = SwitchRequest {
            target: 1,
            generation: 1,
        };
        let (latest, dropped) = latest_request(first, &receiver);
        assert_eq!(latest.generation, 4);
        assert_eq!(latest.target, 4);
        assert_eq!(dropped, 3);
        assert!(receiver.try_recv().is_err(), "older requests are dropped");
    }

    #[test]
    fn test_single_request_is_kept() {
        let (_sender, receiver) = mpsc::channel();
        let first = SwitchRequest {
            target: RU,
            generation: 7,
        };
        let (latest, dropped) = latest_request(first, &receiver);
        assert_eq!(latest.generation, 7);
        assert_eq!(dropped, 0);
    }

    #[test]
    fn test_report_explains_why_fallback_was_skipped() {
        let window = FakeWindow {
            modifiers: true,
            ..FakeWindow::deaf(US)
        };
        let report = perform_switch(&window, RU, &never);
        assert_eq!(report.reason, Some(REASON_MODIFIER));
        assert_eq!(report.presses, 0);

        let busy = FakeWindow {
            busy: true,
            responsive: false,
            ..FakeWindow::new(US)
        };
        assert_eq!(
            perform_switch(&busy, RU, &never).reason,
            Some(REASON_NOT_RESPONDING)
        );
    }

    #[test]
    fn test_report_counts_presses() {
        let report = perform_switch(&FakeWindow::deaf(US), RU, &never);
        assert_eq!(report.outcome, SwitchOutcome::SwitchedByFallback);
        assert_eq!(report.presses, 3);
        assert_eq!(report.reason, None);
        assert!(!report.late);
    }

    #[test]
    fn test_report_marks_late_request() {
        let window = FakeWindow {
            busy: true,
            ..FakeWindow::new(US)
        };
        let report = perform_switch(&window, RU, &never);
        assert_eq!(report.outcome, SwitchOutcome::Switched);
        assert!(report.late);

        let normal = perform_switch(&FakeWindow::new(US), RU, &never);
        assert!(!normal.late);
    }

    #[test]
    fn test_report_tells_focus_moved_apart_from_modifiers() {
        let window = FakeWindow {
            switch_window_after: Some(1),
            ..FakeWindow::deaf(US)
        };
        assert_eq!(
            perform_switch(&window, RU, &never).reason,
            Some(REASON_FOCUS_MOVED)
        );
    }

    #[test]
    fn test_describe_normal_switch() {
        let report = perform_switch(&FakeWindow::new(US), RU, &never);
        assert_eq!(
            describe_switch(
                &report,
                "us",
                "ru",
                6,
                "firefox.exe (MozillaWindowClass)",
                0
            ),
            "us → ru  Switched: 6 ms · firefox.exe (MozillaWindowClass)"
        );
    }

    #[test]
    fn test_describe_fallback_switch() {
        let report = perform_switch(&FakeWindow::deaf(US), RU, &never);
        assert_eq!(
            describe_switch(&report, "us", "ru", 231, "Code.exe (Chrome_WidgetWin_1)", 0),
            "us → ru  SwitchedByFallback: 3× Win+Space, 231 ms · Code.exe (Chrome_WidgetWin_1)"
        );
    }

    #[test]
    fn test_describe_late_and_failed_switches() {
        let late = perform_switch(
            &FakeWindow {
                busy: true,
                ..FakeWindow::new(US)
            },
            RU,
            &never,
        );
        assert_eq!(
            describe_switch(&late, "us", "ru", 180, "idea64.exe (SunAwtFrame)", 0),
            concat!(
                "us → ru  Switched late: applied after the 150 ms check, no Win+Space, 180 ms",
                " · idea64.exe (SunAwtFrame)"
            )
        );

        let failed = perform_switch(
            &FakeWindow {
                modifiers: true,
                ..FakeWindow::deaf(US)
            },
            RU,
            &never,
        );
        assert_eq!(
            describe_switch(&failed, "us", "ru", 150, "app.exe (Cls)", 2),
            concat!(
                "us → ru  Failed: a modifier key is held, no Win+Space, 150 ms · app.exe (Cls)",
                " (+2 earlier press(es) skipped)"
            )
        );
    }

    #[test]
    fn test_fallback_step_done_before_first_press() {
        // Double-switch guard: the target is already there, don't press
        assert_eq!(fallback_step(RU, RU, 0, 4), FallbackStep::Done);
    }

    #[test]
    fn test_fallback_step_presses_until_cap() {
        assert_eq!(fallback_step(US, RU, 0, 4), FallbackStep::PressAgain);
        assert_eq!(fallback_step(US, RU, 3, 4), FallbackStep::PressAgain);
        assert_eq!(fallback_step(US, RU, 4, 4), FallbackStep::GiveUp);
    }

    #[test]
    fn test_fallback_step_done_wins_over_cap() {
        assert_eq!(fallback_step(RU, RU, 4, 4), FallbackStep::Done);
    }

    #[test]
    fn test_win_space_inputs_order_and_flags() {
        let mut inputs = win_space_inputs();
        let expected = [
            (VK_LWIN, false),
            (VK_SPACE, false),
            (VK_SPACE, true),
            (VK_LWIN, true),
        ];
        for (input, (vk, key_up)) in inputs.iter_mut().zip(expected) {
            assert_eq!(input.type_, INPUT_KEYBOARD);
            let ki = unsafe { input.u.ki_mut() };
            assert_eq!(ki.wVk, vk as u16);
            assert_eq!(ki.dwFlags & KEYEVENTF_KEYUP != 0, key_up);
            assert_eq!(ki.dwFlags & KEYEVENTF_EXTENDEDKEY != 0, vk == VK_LWIN);
            assert_eq!(ki.dwExtraInfo, CCAPS_EXTRA_INFO);
        }
    }

    #[test]
    fn test_win_space_release_inputs_release_both_keys() {
        let mut inputs = win_space_release_inputs();
        let ki: Vec<(u16, u32)> = inputs
            .iter_mut()
            .map(|input| {
                let ki = unsafe { input.u.ki_mut() };
                (ki.wVk, ki.dwFlags & KEYEVENTF_KEYUP)
            })
            .collect();
        assert_eq!(
            ki,
            vec![
                (VK_SPACE as u16, KEYEVENTF_KEYUP),
                (VK_LWIN as u16, KEYEVENTF_KEYUP)
            ]
        );
    }

    #[test]
    fn test_fake_cycle_has_distinct_layouts() {
        // Sanity check for the fake: DE is reachable from US within one cycle
        let window = FakeWindow::deaf(US);
        assert_eq!(
            perform_switch(&window, DE, &never),
            SwitchOutcome::SwitchedByFallback
        );
        assert_eq!(window.presses.get(), 2);
    }
}
