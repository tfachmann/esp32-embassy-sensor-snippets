//! SH1106 128x64 OLED on the shared I2C bus (addr 0x3C).

use core::fmt::Write;

use embassy_time::{Duration, Timer};
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Baseline, Text};
use oled_async::builder::Builder;
use oled_async::prelude::*;

use crate::bus::SharedI2c;
use crate::control;

struct FmtBuf {
    buf: [u8; 24],
    len: usize,
}

impl FmtBuf {
    fn new() -> Self {
        Self {
            buf: [0; 24],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

impl Write for FmtBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let b = s.as_bytes();
        let n = b.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&b[..n]);
        self.len += n;
        Ok(())
    }
}

#[embassy_executor::task]
pub async fn run(i2c: SharedI2c) {
    let interface = display_interface_i2c::I2CInterface::new(i2c, 0x3C, 0x40);
    let raw_disp = Builder::new(oled_async::displays::sh1106::Sh1106_128_64 {})
        .with_rotation(DisplayRotation::Rotate0)
        .connect(interface);
    let mut display: GraphicsMode<_, _> = raw_disp.into();

    // Retry init rather than unwrap -- a NACK on a cold boot must not panic
    // (that would freeze everything). Give up gracefully if no display.
    let mut ready = false;
    for _ in 0..20 {
        if display.init().await.is_ok() {
            ready = true;
            break;
        }
        log::warn!("display init failed, retrying...");
        Timer::after(Duration::from_millis(100)).await;
    }
    if !ready {
        log::error!("no display connected; continuing without display");
        return;
    }

    // SH1106 GDDRAM holds garbage on power-up.
    display.clear();
    let _ = display.flush().await;

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();
    let box_style = PrimitiveStyle::with_fill(BinaryColor::On);

    let (w, h) = display.get_dimensions();
    let w = w as i32;
    let box_w = 10;
    let track = w - box_w;

    let mut frame: i32 = 0;
    loop {
        let phase = frame % (2 * track);
        let x = if phase < track {
            phase
        } else {
            2 * track - phase
        };

        let mut speed_line = FmtBuf::new();
        let _ = write!(speed_line, "Speed: {}", control::speed());
        let mut mode_line = FmtBuf::new();
        let _ = write!(mode_line, "Mode:  {}", control::mode_name(control::mode()));
        let mut imu_line = FmtBuf::new();
        let _ = write!(imu_line, "Tilt P:{} R:{}", control::pitch(), control::roll());

        display.clear();
        let _ = Text::with_baseline("Doktorhut Flo", Point::new(0, 0), text_style, Baseline::Top)
            .draw(&mut display);
        let _ = Text::with_baseline(speed_line.as_str(), Point::new(0, 14), text_style, Baseline::Top)
            .draw(&mut display);
        let _ = Text::with_baseline(mode_line.as_str(), Point::new(0, 26), text_style, Baseline::Top)
            .draw(&mut display);
        let _ = Text::with_baseline(imu_line.as_str(), Point::new(0, 38), text_style, Baseline::Top)
            .draw(&mut display);
        let _ = Rectangle::new(Point::new(x, h as i32 - 10), Size::new(box_w as u32, 8))
            .into_styled(box_style)
            .draw(&mut display);
        // Ignore flush errors; the next frame retries.
        let _ = display.flush().await;

        frame = frame.wrapping_add(2);
        Timer::after(Duration::from_millis(40)).await;
    }
}
