//! WS2812B driver over RMT (blocking, streaming — not limited by channel RAM).
//!
//! `Rmt` is created once via [`new_rmt`]; each strip then takes a different RMT
//! channel. Channels must be spaced by `memsize` (no block overlap). With
//! `MEMSIZE = 2`: channels 0/2/4/6 (blocks 0-1, 2-3, 4-5, 6-7) -> up to 4
//! strips. The ESP32 has 8 RMT blocks total.

use core::sync::atomic::{AtomicUsize, Ordering};

use esp_hal::gpio::interconnect::PeripheralOutput;
use esp_hal::gpio::Level;
use esp_hal::peripherals::RMT;
use esp_hal::rmt::{
    AnyTxChannel, PulseCode, RawChannelAccess, Rmt, Tx, TxChannel, TxChannelConfig,
    TxChannelCreator,
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

// Pulse buffers live in static memory, not the embassy task arena: each one is
// ~`NUM_LEDS * 96` bytes and would otherwise blow the arena. Bump MAX_STRIPS to
// drive more strips.
const MAX_STRIPS: usize = 4;
const MEMSIZE: u8 = 2;
static mut PULSE_BUFS: [[u32; BUF_LEN]; MAX_STRIPS] = [[0; BUF_LEN]; MAX_STRIPS];
static NEXT_BUF: AtomicUsize = AtomicUsize::new(0);

fn claim_buf() -> &'static mut [u32; BUF_LEN] {
    let idx = NEXT_BUF.fetch_add(1, Ordering::Relaxed);
    assert!(idx < MAX_STRIPS, "more strips than MAX_STRIPS");
    // SAFETY: each idx is handed out exactly once, so this &mut is unique.
    unsafe { &mut *(&raw mut PULSE_BUFS[idx]) }
}

/// Create the shared RMT peripheral at the 80 MHz the timing constants assume.
pub fn new_rmt(rmt: RMT<'static>) -> Rmt<'static, Blocking> {
    Rmt::new(rmt, Rate::from_mhz(80)).unwrap()
}

pub struct Ws2812 {
    // Option: transmit() consumes the channel, wait() hands it back.
    channel: Option<AnyTxChannel<Blocking>>,
    pulse: &'static mut [u32; BUF_LEN],
}

impl Ws2812 {
    pub fn new<'d, C>(creator: C, data: impl PeripheralOutput<'d>) -> Self
    where
        C: TxChannelCreator<'d, Blocking>,
        C::Raw: RawChannelAccess<Dir = Tx>,
    {
        let channel = creator
            .configure_tx(
                data,
                TxChannelConfig::default()
                    .with_clk_divider(1)
                    .with_idle_output(true)
                    .with_idle_output_level(Level::Low)
                    .with_memsize(MEMSIZE),
            )
            .unwrap()
            .degrade();

        Self {
            channel: Some(channel),
            pulse: claim_buf(),
        }
    }

    pub fn write(&mut self, fb: &Framebuffer) {
        encode(self.pulse, fb);

        let ch = self.channel.take().expect("rmt channel present");
        let tx = ch.transmit(&self.pulse[..]).expect("rmt transmit");
        self.channel = Some(match tx.wait() {
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
