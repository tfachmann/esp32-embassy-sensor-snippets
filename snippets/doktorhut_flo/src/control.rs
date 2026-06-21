//! Shared control state, written by the rotary encoder, read by led_strip and
//! display. Single writer per field, so plain atomic load/store is enough.

use core::sync::atomic::{AtomicI32, AtomicU32, Ordering::Relaxed};

pub const SPEED_MIN: u32 = 1;
pub const SPEED_MAX: u32 = 20;

pub const MODE_COUNT: u32 = 2;

pub const VOLUME_MIN: u32 = 0;
pub const VOLUME_MAX: u32 = 30;
pub const BRIGHTNESS_MIN: u32 = 1;
pub const BRIGHTNESS_MAX: u32 = 8; // brightness level; higher = brighter

// Packet velocity in Q8.8 LEDs-per-frame, interpolated across the speed range.
const VEL_MIN_Q8: u32 = 8; // ~0.03 LED/frame
const VEL_MAX_Q8: u32 = 1024; // 4 LED/frame

static SPEED: AtomicU32 = AtomicU32::new(6);
static MODE: AtomicU32 = AtomicU32::new(0);
static VOLUME: AtomicU32 = AtomicU32::new(15);
static BRIGHTNESS: AtomicU32 = AtomicU32::new(5);

// IMU orientation (deg), written by the imu task, read by the display.
static PITCH: AtomicI32 = AtomicI32::new(0);
static ROLL: AtomicI32 = AtomicI32::new(0);

// IMU acceleration (g, f32 bits), written by the imu task.
static ACCEL_X: AtomicU32 = AtomicU32::new(0);
static ACCEL_Y: AtomicU32 = AtomicU32::new(0);
static ACCEL_Z: AtomicU32 = AtomicU32::new(0);

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

pub fn prev_mode() {
    MODE.store((mode() + MODE_COUNT - 1) % MODE_COUNT, Relaxed);
}

pub fn mode_name(mode: u32) -> &'static str {
    match mode {
        0 => "Packet",
        1 => "Stream",
        _ => "?",
    }
}

pub fn volume() -> u32 {
    VOLUME.load(Relaxed)
}

pub fn volume_up() {
    VOLUME.store((volume() + 1).min(VOLUME_MAX), Relaxed);
}

pub fn volume_down() {
    VOLUME.store(volume().saturating_sub(1).max(VOLUME_MIN), Relaxed);
}

pub fn brightness_level() -> u32 {
    BRIGHTNESS.load(Relaxed)
}

pub fn brighter() {
    BRIGHTNESS.store((brightness_level() + 1).min(BRIGHTNESS_MAX), Relaxed);
}

pub fn dimmer() {
    BRIGHTNESS.store(brightness_level().saturating_sub(1).max(BRIGHTNESS_MIN), Relaxed);
}

/// Right-shift applied per color channel: level 8 -> 0 (full), level 1 -> 7 (dim).
pub fn brightness_shift() -> u8 {
    (BRIGHTNESS_MAX - brightness_level()) as u8
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

pub fn set_accel(ax: f32, ay: f32, az: f32) {
    ACCEL_X.store(ax.to_bits(), Relaxed);
    ACCEL_Y.store(ay.to_bits(), Relaxed);
    ACCEL_Z.store(az.to_bits(), Relaxed);
}

pub fn accel_x() -> f32 {
    f32::from_bits(ACCEL_X.load(Relaxed))
}

pub fn accel_y() -> f32 {
    f32::from_bits(ACCEL_Y.load(Relaxed))
}

pub fn accel_z() -> f32 {
    f32::from_bits(ACCEL_Z.load(Relaxed))
}
