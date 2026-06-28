//! WS2812B LED strip: visualize bytes travelling down a wire.

mod driver;
pub mod effects;

use embassy_time::{Duration, Instant, Timer};

use crate::control;
pub use driver::{new_rmt, Ws2812};
use effects::{BeerByte, Effect, Stream};

/// Compile-time strip length. The pulse buffer (`NUM_LEDS * 24 + 1` u32) lives
/// in the task arena; the default 20480 holds up to ~100 LEDs.
pub const NUM_LEDS: usize = 60;

/// RGB pixel; driver reorders to WS2812B GRB.
pub type Rgb = [u8; 3];
pub type Framebuffer = [Rgb; NUM_LEDS];

const FRAME_MS: u64 = 8;

const BEER_BYTE: u8 = 0b1011_0010;
// Beer byte travels at this fraction of the configured LED speed (slower than
// the stream, but still scales with it -> servo triggers earlier when faster).
const BEER_SPEED_DIV: u32 = 4;
// Beer byte velocity (Q8.8 LEDs/frame) in BEER MANUAL: fast, but not instant.
const BEER_MANUAL_VEL_Q8: i32 = 4 * 256;

/// Each physical strip is dedicated to one process.
#[derive(Clone, Copy, PartialEq)]
pub enum StripRole {
    Beer,
    Imu,
    Music,
}

const MUSIC_COLOR: Rgb = [0, 255, 90]; // green pulse while music plays
const MUSIC_PERIOD_MS: u32 = 2000; // breathing cycle

#[embassy_executor::task(pool_size = 3)]
pub async fn run(mut strip: Ws2812, role: StripRole) {
    let mut fb: Framebuffer = [[0, 0, 0]; NUM_LEDS];

    let mut stream = Stream::new();
    let mut beer = BeerByte::new(BEER_BYTE);
    let mut beer_was_on = false;

    loop {
        fb.fill([0, 0, 0]);

        match role {
            // IMU strip: the byte stream while the IMU is on, brightness ramping
            // up after it starts.
            StripRole::Imu => {
                if control::imu_on() {
                    stream.set_velocity_q8(control::velocity_q8());
                    stream.render(&mut fb);
                    let ramp = control::imu_ramp_q8(Instant::now().as_millis() as u32);
                    if ramp < 256 {
                        for px in fb.iter_mut() {
                            for c in px.iter_mut() {
                                *c = (*c as u32 * ramp / 256) as u8;
                            }
                        }
                    }
                }
            }

            // BEER strip: a single distinct-color byte travelling the strip; it
            // also signals the servo when it reaches the end.
            StripRole::Beer => {
                let beer_on = control::beer_on();
                if beer_on && !beer_was_on {
                    beer.reset(); // rising edge -> fire a new shot
                }
                beer_was_on = beer_on;
                if beer_on {
                    let beer_vel = if control::manual_on() {
                        BEER_MANUAL_VEL_Q8
                    } else {
                        (control::velocity_q8() / BEER_SPEED_DIV).max(1) as i32
                    };
                    beer.overlay(&mut fb, beer_vel);
                    if beer.finished() {
                        control::signal_beer_arrived();
                        control::clear_beer();
                    }
                }
            }

            // MUSIC strip: a gentle breathing pulse while music plays. Phase from
            // wall-clock time (not a per-frame counter), so it stays smooth even if
            // core1's frame cadence stutters. (Placeholder until the multi-track
            // music refactor.)
            StripRole::Music => {
                if control::music_on() {
                    let t = Instant::now().as_millis() as u32;
                    let phase = (t % MUSIC_PERIOD_MS) * 256 / MUSIC_PERIOD_MS; // 0..255
                    let tri = if phase < 128 { phase * 2 } else { (255 - phase) * 2 };
                    let level = 40 + tri * 215 / 255; // keep a dim floor
                    let color = [
                        (MUSIC_COLOR[0] as u32 * level / 255) as u8,
                        (MUSIC_COLOR[1] as u32 * level / 255) as u8,
                        (MUSIC_COLOR[2] as u32 * level / 255) as u8,
                    ];
                    fb.fill(color);
                }
            }
        }

        strip.write(&fb);
        Timer::after(Duration::from_millis(FRAME_MS)).await;
    }
}
