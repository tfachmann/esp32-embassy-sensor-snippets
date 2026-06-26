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

#[embassy_executor::task(pool_size = 4)]
pub async fn run(mut strip: Ws2812) {
    let mut fb: Framebuffer = [[0, 0, 0]; NUM_LEDS];

    let mut stream = Stream::new();
    let mut beer = BeerByte::new(BEER_BYTE);
    let mut beer_was_on = false;

    loop {
        // LEDs are process-driven: IMU on -> stream; BEER -> a distinct-color
        // byte overlaid on top; nothing running -> blank.
        fb.fill([0, 0, 0]);

        if control::imu_on() {
            stream.set_velocity_q8(control::velocity_q8());
            stream.render(&mut fb);
            // Ramp the stream brightness up after the IMU starts.
            let ramp = control::imu_ramp_q8(Instant::now().as_millis() as u32);
            if ramp < 256 {
                for px in fb.iter_mut() {
                    for c in px.iter_mut() {
                        *c = (*c as u32 * ramp / 256) as u8;
                    }
                }
            }
        }

        let beer_on = control::beer_on();
        if beer_on && !beer_was_on {
            beer.reset(); // rising edge -> fire a new shot
        }
        beer_was_on = beer_on;
        if beer_on {
            let beer_vel = (control::velocity_q8() / BEER_SPEED_DIV).max(1) as i32;
            beer.overlay(&mut fb, beer_vel);
            if beer.finished() {
                // The byte reached the strip end -> the servo (the beer tap).
                control::signal_beer_arrived();
                control::clear_beer();
            }
        }

        strip.write(&fb);
        Timer::after(Duration::from_millis(FRAME_MS)).await;
    }
}
