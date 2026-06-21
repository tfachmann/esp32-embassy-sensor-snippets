//! SH1106 128x64 OLED on the shared I2C bus (addr 0x3C).

use core::fmt::Write;

use embassy_time::{Duration, Timer};
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::{Baseline, Text};
use oled_async::builder::Builder;
use oled_async::prelude::*;

use crate::bus::SharedI2c;
use crate::{control, ui};

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

    loop {
        let view = ui::view();

        display.clear();
        if view.in_controls {
            draw_text(&mut display, text_style, "= CONTROLS =", 0);
            for (i, name) in ui::CONTROL_ITEMS.iter().enumerate() {
                let y = 12 + i as i32 * 10;
                let cur = i == view.cursor;
                let marker = if cur {
                    if view.editing {
                        "*"
                    } else {
                        ">"
                    }
                } else {
                    " "
                };
                match i {
                    0 => row_bar(&mut display, text_style, marker, "Vol", y,
                        control::volume(), control::VOLUME_MIN, control::VOLUME_MAX),
                    1 => row_bar(&mut display, text_style, marker, "Spd", y,
                        control::speed(), control::SPEED_MIN, control::SPEED_MAX),
                    2 => row_bar(&mut display, text_style, marker, "Brt", y,
                        control::brightness_level(), control::BRIGHTNESS_MIN, control::BRIGHTNESS_MAX),
                    3 => {
                        let mut l = FmtBuf::new();
                        let _ = write!(l, "{} Fx: {}", marker, control::mode_name(control::mode()));
                        draw_text(&mut display, text_style, l.as_str(), y);
                    }
                    _ => {
                        let mut l = FmtBuf::new();
                        let _ = write!(l, "{} {}", marker, name);
                        draw_text(&mut display, text_style, l.as_str(), y);
                    }
                }
            }
        } else {
            draw_text(&mut display, text_style, "== MENU ==", 0);
            for (i, name) in ui::MAIN_ITEMS.iter().enumerate() {
                let y = 16 + i as i32 * 14;
                let marker = if i == view.cursor { ">" } else { " " };
                let mut l = FmtBuf::new();
                let _ = write!(l, "{} {}", marker, name);
                draw_text(&mut display, text_style, l.as_str(), y);
            }
        }
        // Ignore flush errors; the next frame retries.
        let _ = display.flush().await;

        Timer::after(Duration::from_millis(40)).await;
    }
}

fn draw_text<D>(display: &mut D, style: MonoTextStyle<'_, BinaryColor>, text: &str, y: i32)
where
    D: DrawTarget<Color = BinaryColor>,
{
    let _ = Text::with_baseline(text, Point::new(0, y), style, Baseline::Top).draw(display);
}

/// A control row: "<marker> <label>" on the left, a value bar on the right.
fn row_bar<D>(
    display: &mut D,
    style: MonoTextStyle<'_, BinaryColor>,
    marker: &str,
    label: &str,
    y: i32,
    value: u32,
    min: u32,
    max: u32,
) where
    D: DrawTarget<Color = BinaryColor>,
{
    let mut l = FmtBuf::new();
    let _ = write!(l, "{} {}", marker, label);
    draw_text(display, style, l.as_str(), y);
    draw_bar(display, 42, y + 1, 82, 7, value, min, max);
}

fn draw_bar<D>(display: &mut D, x: i32, y: i32, w: i32, h: i32, value: u32, min: u32, max: u32)
where
    D: DrawTarget<Color = BinaryColor>,
{
    let outline = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::On)
        .stroke_width(1)
        .build();
    let _ = Rectangle::new(Point::new(x, y), Size::new(w as u32, h as u32))
        .into_styled(outline)
        .draw(display);

    let span = max.saturating_sub(min).max(1);
    let fill = (w - 2).max(0) as u32 * value.saturating_sub(min) / span;
    if fill > 0 {
        let _ = Rectangle::new(Point::new(x + 1, y + 1), Size::new(fill, (h - 2) as u32))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display);
    }
}
