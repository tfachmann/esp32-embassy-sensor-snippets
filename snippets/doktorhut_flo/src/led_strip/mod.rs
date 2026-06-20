//! WS2812B LED strip: visualize bytes travelling down a wire.

mod driver;
pub mod effects;

use embassy_time::{Duration, Timer};
use esp_hal::peripherals::{GPIO5, RMT};

use crate::control;
pub use driver::Ws2812;
use effects::{Effect, Packet};

/// Compile-time strip length. The pulse buffer (`NUM_LEDS * 24 + 1` u32) lives
/// in the task arena; the default 20480 holds up to ~100 LEDs.
pub const NUM_LEDS: usize = 60;

/// Global brightness as a right-shift (0 = full, 3 = 1/8).
pub const BRIGHTNESS_SHIFT: u8 = 3;

/// RGB pixel; driver reorders to WS2812B GRB.
pub type Rgb = [u8; 3];
pub type Framebuffer = [Rgb; NUM_LEDS];

const FRAME_MS: u64 = 8;

const PACKET_BYTE: u8 = 0b1011_0010;
const PACKET_BIT_WIDTH: u32 = 1;

#[embassy_executor::task]
pub async fn run(rmt: RMT<'static>, data: GPIO5<'static>) {
    let mut strip = Ws2812::new(rmt, data);
    let mut fb: Framebuffer = [[0, 0, 0]; NUM_LEDS];

    let mut effect = Packet::new(PACKET_BYTE).with_bit_width(PACKET_BIT_WIDTH);

    loop {
        effect.set_velocity_q8(control::velocity_q8());
        if !control::is_paused() {
            effect.render(&mut fb);
            strip.write(&fb);
        }
        Timer::after(Duration::from_millis(FRAME_MS)).await;
    }
}
