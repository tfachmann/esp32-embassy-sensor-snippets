//! Shared control state, written by the rotary encoder, read by led_strip and
//! display. Single writer per field, so plain atomic load/store is enough.

use core::sync::atomic::{AtomicI32, AtomicU32, Ordering::Relaxed};

pub const SPEED_MIN: u32 = 1;
pub const SPEED_MAX: u32 = 20;

pub const MODE_COUNT: u32 = 2;

// Packet velocity in Q8.8 LEDs-per-frame, interpolated across the speed range.
const VEL_MIN_Q8: u32 = 8; // ~0.03 LED/frame
const VEL_MAX_Q8: u32 = 1024; // 4 LED/frame

static SPEED: AtomicU32 = AtomicU32::new(6);
static MODE: AtomicU32 = AtomicU32::new(0);

// IMU orientation (deg), written by the imu task, read by the display.
static PITCH: AtomicI32 = AtomicI32::new(0);
static ROLL: AtomicI32 = AtomicI32::new(0);

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

pub fn mode() -> u32 {
    MODE.load(Relaxed)
}

pub fn next_mode() {
    MODE.store((mode() + 1) % MODE_COUNT, Relaxed);
}

pub fn mode_name(mode: u32) -> &'static str {
    match mode {
        0 => "Packet",
        1 => "Stream",
        _ => "?",
    }
}

pub fn set_orientation(pitch: i32, roll: i32) {
    PITCH.store(pitch, Relaxed);
    ROLL.store(roll, Relaxed);
}

pub fn pitch() -> i32 {
    PITCH.load(Relaxed)
}

pub fn roll() -> i32 {
    ROLL.load(Relaxed)
}
