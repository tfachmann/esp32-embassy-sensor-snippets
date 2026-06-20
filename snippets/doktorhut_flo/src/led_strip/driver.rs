//! WS2812B driver over RMT (blocking, streaming — not limited by channel RAM).

use esp_hal::gpio::Level;
use esp_hal::peripherals::{GPIO5, RMT};
use esp_hal::rmt::{
    AnyTxChannel, PulseCode, Rmt, TxChannel, TxChannelConfig, TxChannelCreator,
};
use esp_hal::time::Rate;
use esp_hal::Blocking;

use super::{Framebuffer, BRIGHTNESS_SHIFT, NUM_LEDS};

// RMT at 80 MHz, divider 1 -> 12.5 ns/tick. WS2812B timing (+-150 ns):
const T0H: u16 = 32; // 0.40 us
const T0L: u16 = 68; // 0.85 us
const T1H: u16 = 64; // 0.80 us
const T1L: u16 = 36; // 0.45 us

const BUF_LEN: usize = NUM_LEDS * 24 + 1;

pub struct Ws2812 {
    // Option: transmit() consumes the channel, wait() hands it back.
    channel: Option<AnyTxChannel<Blocking>>,
    pulse: [u32; BUF_LEN],
}

impl Ws2812 {
    pub fn new(rmt: RMT<'static>, data: GPIO5<'static>) -> Self {
        let rmt = Rmt::new(rmt, Rate::from_mhz(80)).unwrap();
        let channel = rmt
            .channel0
            .configure_tx(
                data,
                TxChannelConfig::default()
                    .with_clk_divider(1)
                    .with_idle_output(true)
                    .with_idle_output_level(Level::Low)
                    .with_memsize(4),
            )
            .unwrap()
            .degrade();

        Self {
            channel: Some(channel),
            pulse: [PulseCode::empty(); BUF_LEN],
        }
    }

    pub fn write(&mut self, fb: &Framebuffer) {
        let Self { channel, pulse } = self;
        encode(pulse, fb);

        let ch = channel.take().expect("rmt channel present");
        let tx = ch.transmit(&pulse[..]).expect("rmt transmit");
        *channel = Some(match tx.wait() {
            Ok(ch) => ch,
            Err((_e, ch)) => ch,
        });
    }
}

// GRB order, MSB first, with brightness shift applied.
fn encode(buf: &mut [u32; BUF_LEN], fb: &Framebuffer) {
    let mut i = 0;
    for &[r, g, b] in fb.iter() {
        let r = r >> BRIGHTNESS_SHIFT;
        let g = g >> BRIGHTNESS_SHIFT;
        let b = b >> BRIGHTNESS_SHIFT;
        for byte in [g, r, b] {
            for bit in (0..8).rev() {
                buf[i] = if (byte >> bit) & 1 == 1 {
                    PulseCode::new(Level::High, T1H, Level::Low, T1L)
                } else {
                    PulseCode::new(Level::High, T0H, Level::Low, T0L)
                };
                i += 1;
            }
        }
    }
    buf[i] = PulseCode::empty();
}
