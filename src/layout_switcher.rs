// Layout switching with verification, and a Win+Space fallback for windows that ignore
// WM_INPUTLANGCHANGEREQUEST.
//
// The decision logic works through the SwitchOps trait, so it can be tested against a
// fake window without touching WinAPI.

// Wired into the hook in a later change
#![allow(dead_code)]

use crate::keyboard_hook::CCAPS_EXTRA_INFO;
use crate::layout_manager;
use std::mem;
use std::time::Duration;
use winapi::um::winuser::{
    GetAsyncKeyState, GetForegroundWindow, SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_SPACE,
};

// How long the window gets to apply WM_INPUTLANGCHANGEREQUEST before CCaps falls back
// to Win+Space (measured: 5-26 ms)
pub const VERIFY_TIMEOUT_MS: u32 = 150;
// How long to wait for the layout to change after one Win+Space (measured: 5-11 ms)
pub const PRESS_TIMEOUT_MS: u32 = 100;
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
pub struct SystemOps;

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

// Switches the foreground window to `target`: posts the request, verifies it, and if the
// window ignored it, presses Win+Space until the target is reached. `superseded` tells
// whether a newer request is waiting.
pub fn perform_switch(
    ops: &impl SwitchOps,
    target: usize,
    superseded: &dyn Fn() -> bool,
) -> SwitchOutcome {
    let window = ops.foreground();
    if !ops.post_request(target) {
        return SwitchOutcome::Failed;
    }

    let Some(current) = poll_layout(ops, VERIFY_TIMEOUT_MS, |layout| layout == target) else {
        return SwitchOutcome::Unverified;
    };
    if current == target {
        return SwitchOutcome::Switched;
    }

    // The window ignored the request (or hasn't applied it yet)
    if superseded() {
        return SwitchOutcome::Superseded;
    }
    if ops.modifiers_held() || !ops.is_installed(target) {
        return SwitchOutcome::Failed;
    }
    if !ops.target_responsive() {
        return SwitchOutcome::Unverified;
    }

    // Win+Space cycles through every installed layout, in an order CCaps can't know in
    // advance: press and re-read until the target, at most one full cycle. The first
    // read happens before any press, so a request applied late counts as Switched.
    let max_presses = ops.installed_layout_count();
    let mut presses = 0;
    loop {
        let Some(current) = ops.current_layout() else {
            return SwitchOutcome::Unverified;
        };
        match fallback_step(current, target, presses, max_presses) {
            FallbackStep::Done if presses == 0 => return SwitchOutcome::Switched,
            FallbackStep::Done => return SwitchOutcome::SwitchedByFallback,
            FallbackStep::GiveUp => return SwitchOutcome::Failed,
            FallbackStep::PressAgain => {}
        }
        if superseded() {
            return SwitchOutcome::Superseded;
        }
        if ops.foreground() != window || ops.modifiers_held() {
            return SwitchOutcome::Failed;
        }
        if !ops.press_win_space() {
            return SwitchOutcome::Failed;
        }
        presses += 1;
        // No change after a press: it was blocked or is still on its way. Pressing again
        // could overshoot the target once it lands, so stop here.
        match poll_layout(ops, PRESS_TIMEOUT_MS, |layout| layout != current) {
            Some(layout) if layout != current => {}
            _ => return SwitchOutcome::Unverified,
        }
    }
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
            if !self.ignores_win_space {
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
        // The layout didn't change after the first press, so CCaps stops right there
        assert_eq!(window.presses.get(), 1);
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
