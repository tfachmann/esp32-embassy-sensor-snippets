//! Shared control state, written by the rotary encoder, read by led_strip and
//! display. Single writer per field, so plain atomic load/store is enough.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

pub const SPEED_MIN: u32 = 1;
pub const SPEED_MAX: u32 = 20;

// Packet velocity in Q8.8 LEDs-per-frame, interpolated across the speed range.
const VEL_MIN_Q8: u32 = 8; // ~0.03 LED/frame
const VEL_MAX_Q8: u32 = 1024; // 4 LED/frame

static SPEED: AtomicU32 = AtomicU32::new(6);
static PAUSED: AtomicBool = AtomicBool::new(false);

pub fn speed() -> u32 {
    SPEED.load(Relaxed)
}

pub fn velocity_q8() -> u32 {
    let s = speed() - SPEED_MIN;
    let span = SPEED_MAX - SPEED_MIN;
    VEL_MIN_Q8 + (VEL_MAX_Q8 - VEL_MIN_Q8) * s / span
}

pub fn faster() {
    SPEED.store((speed() + 1).min(SPEED_MAX), Relaxed);
}

pub fn slower() {
    SPEED.store(speed().saturating_sub(1).max(SPEED_MIN), Relaxed);
}

pub fn is_paused() -> bool {
    PAUSED.load(Relaxed)
}

pub fn toggle_pause() {
    PAUSED.store(!is_paused(), Relaxed);
}
