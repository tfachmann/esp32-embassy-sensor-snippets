//! Shared control state, written by the rotary encoder, read by led_strip and
//! display. Single writer per field, so plain atomic load/store is enough.

use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering::Relaxed};

pub const SPEED_MIN: u32 = 1;
pub const SPEED_MAX: u32 = 20;

pub const VOLUME_MIN: u32 = 0;
pub const VOLUME_MAX: u32 = 30;
pub const BRIGHTNESS_MIN: u32 = 1;
pub const BRIGHTNESS_MAX: u32 = 8; // brightness level; higher = brighter

// Packet velocity in Q8.8 LEDs-per-frame, interpolated across the speed range.
const VEL_MIN_Q8: u32 = 8; // ~0.03 LED/frame
const VEL_MAX_Q8: u32 = 1024; // 4 LED/frame

static SPEED: AtomicU32 = AtomicU32::new(6);
static VOLUME: AtomicU32 = AtomicU32::new(24); // 80% of VOLUME_MAX (30)
static BRIGHTNESS: AtomicU32 = AtomicU32::new(5);

// Process running state (all off at boot). Indices for process_running():
// 0=beer, 1=music, 2=imu, 3=fluids, 4=tilt.
static BEER_ON: AtomicBool = AtomicBool::new(false);
// Set by led_strip when the beer byte reaches the end of the strip (= arrives
// at the servo); consumed by the servo task to start its sequence.
static BEER_ARRIVED: AtomicBool = AtomicBool::new(false);
// True while the servo runs the automatic pour sequence (keeps the BEER
// process indicator lit for the whole pour, not just the LED byte).
static BEER_POURING: AtomicBool = AtomicBool::new(false);
static MUSIC_ON: AtomicBool = AtomicBool::new(false);
static IMU_ON: AtomicBool = AtomicBool::new(false);
static FLUIDS_ON: AtomicBool = AtomicBool::new(false);
static TILT_ON: AtomicBool = AtomicBool::new(false);
static MANUAL_ON: AtomicBool = AtomicBool::new(false);

// Manual servo target (0..4095), driven by the encoder in the BEER MANUAL screen.
pub const SERVO_MIN: u32 = 0;
pub const SERVO_MAX: u32 = 4095;
const SERVO_STEP: u32 = 40;
static SERVO_POS: AtomicU32 = AtomicU32::new(2048);

// IMU brightness ramp: ms timestamp when the IMU turned on (0 = off/not ramping).
pub const IMU_RAMP_MS: u32 = 2500;
static IMU_STARTED_MS: AtomicU32 = AtomicU32::new(0);

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

// --- process state ---------------------------------------------------------

pub fn beer_on() -> bool {
    BEER_ON.load(Relaxed)
}
pub fn start_beer() {
    BEER_ON.store(true, Relaxed);
}
pub fn clear_beer() {
    BEER_ON.store(false, Relaxed);
}

/// led_strip signals that the beer byte reached the strip end (the servo).
pub fn signal_beer_arrived() {
    BEER_ARRIVED.store(true, Relaxed);
}

/// Servo consumes the arrival event (returns true once per arrival, then clears).
pub fn take_beer_arrived() -> bool {
    BEER_ARRIVED.swap(false, Relaxed)
}

pub fn set_beer_pouring(on: bool) {
    BEER_POURING.store(on, Relaxed);
}

pub fn music_on() -> bool {
    MUSIC_ON.load(Relaxed)
}
pub fn toggle_music() {
    MUSIC_ON.store(!music_on(), Relaxed);
}

pub fn imu_on() -> bool {
    IMU_ON.load(Relaxed)
}
pub fn toggle_imu(now_ms: u32) {
    set_imu(!imu_on(), now_ms);
}
pub fn set_imu(on: bool, now_ms: u32) {
    let was = imu_on();
    IMU_ON.store(on, Relaxed);
    if on && !was {
        IMU_STARTED_MS.store(now_ms.max(1), Relaxed); // start the ramp
    } else if !on {
        IMU_STARTED_MS.store(0, Relaxed);
    }
}

/// IMU fully started (on and past its ramp).
pub fn imu_ready(now_ms: u32) -> bool {
    imu_on() && imu_ramp_q8(now_ms) >= 256
}

/// Stream brightness ramp 0..=256 over IMU_RAMP_MS after the IMU turns on.
pub fn imu_ramp_q8(now_ms: u32) -> u32 {
    let started = IMU_STARTED_MS.load(Relaxed);
    if started == 0 {
        return 0;
    }
    let dt = now_ms.wrapping_sub(started);
    if dt >= IMU_RAMP_MS {
        256
    } else {
        dt * 256 / IMU_RAMP_MS
    }
}

pub fn set_fluids_active(on: bool) {
    FLUIDS_ON.store(on, Relaxed);
}
pub fn set_tilt_active(on: bool) {
    TILT_ON.store(on, Relaxed);
}

pub fn manual_on() -> bool {
    MANUAL_ON.load(Relaxed)
}
pub fn set_manual_active(on: bool) {
    MANUAL_ON.store(on, Relaxed);
}

pub fn servo_pos() -> u32 {
    SERVO_POS.load(Relaxed)
}
pub fn servo_step(up: bool) {
    let p = servo_pos();
    let n = if up {
        (p + SERVO_STEP).min(SERVO_MAX)
    } else {
        p.saturating_sub(SERVO_STEP).max(SERVO_MIN)
    };
    SERVO_POS.store(n, Relaxed);
}

/// Running state of process `i`, matching MAIN_ITEMS order
/// (0=beer,1=manual,2=music,3=imu,4=fluids,5=tilt).
pub fn process_running(i: usize) -> bool {
    match i {
        0 => beer_on() || BEER_POURING.load(Relaxed),
        1 => manual_on(),
        2 => music_on(),
        3 => imu_on(),
        4 => FLUIDS_ON.load(Relaxed),
        5 => TILT_ON.load(Relaxed),
        _ => false,
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
