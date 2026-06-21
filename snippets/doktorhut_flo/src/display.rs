//! SH1106 128x64 OLED on the shared I2C bus (addr 0x3C).

use core::fmt::Write;

use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::mono_font::ascii::{FONT_4X6, FONT_5X8, FONT_6X10};
use embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{
    PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, RoundedRectangle,
};
use embedded_graphics::text::{Baseline, Text};
use oled_async::builder::Builder;
use oled_async::prelude::*;

use crate::bus::SharedI2c;
use crate::ui::ViewScreen;
use crate::{about, control, fluid, tilt3d, ui};

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
    // Tiny font for the windowed menu chrome (normal + inverted for highlights).
    let small = MonoTextStyleBuilder::new()
        .font(&FONT_4X6)
        .text_color(BinaryColor::On)
        .build();
    // Slightly larger font for the process list (normal + inverted).
    let med = MonoTextStyleBuilder::new()
        .font(&FONT_5X8)
        .text_color(BinaryColor::On)
        .build();
    let med_inv = MonoTextStyleBuilder::new()
        .font(&FONT_5X8)
        .text_color(BinaryColor::Off)
        .build();

    // FLIP fluid scene lives in a static (it is tens of KB) -- init once.
    let scene = fluid::init();

    loop {
        let now = Instant::now().as_millis() as u32;
        let view = ui::view();

        display.clear();
        match view.screen {
            ViewScreen::Controls => {
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
                        _ => {
                            let mut l = FmtBuf::new();
                            let _ = write!(l, "{} {}", marker, name);
                            draw_text(&mut display, text_style, l.as_str(), y);
                        }
                    }
                }
            }
            ViewScreen::Fluids => {
                fluid::step_and_render(scene, &mut display, control::accel_x(), control::accel_y());
            }
            ViewScreen::About => {
                about::render(&mut display, text_style, small);
            }
            ViewScreen::Tilt => {
                tilt3d::render(
                    &mut display,
                    control::pitch(),
                    control::roll(),
                    control::accel_x(),
                    control::accel_y(),
                    control::accel_z(),
                    text_style,
                );
            }
            ViewScreen::Main => {
                let stroke = PrimitiveStyleBuilder::new()
                    .stroke_color(BinaryColor::On)
                    .stroke_width(1)
                    .build();
                let fill = PrimitiveStyle::with_fill(BinaryColor::On);

                // Window border + title bar.
                let _ = RoundedRectangle::with_equal_corners(
                    Rectangle::new(Point::new(0, 0), Size::new(128, 64)),
                    Size::new(6, 6),
                )
                .into_styled(stroke)
                .draw(&mut display);
                let _ = Text::with_baseline("LN 2.8.2", Point::new(48, 2), small, Baseline::Top)
                    .draw(&mut display);

                // Processes box: selected row is inverted (no arrow).
                let _ = Rectangle::new(Point::new(4, 11), Size::new(66, 49))
                    .into_styled(stroke)
                    .draw(&mut display);
                for i in 0..ui::PROCESS_COUNT {
                    let y = 13 + i as i32 * 8;
                    let name = ui::MAIN_ITEMS[i];
                    if i == view.cursor {
                        let _ = Rectangle::new(Point::new(5, y - 1), Size::new(64, 9))
                            .into_styled(fill)
                            .draw(&mut display);
                        let _ = Text::with_baseline(name, Point::new(7, y), med_inv, Baseline::Top)
                            .draw(&mut display);
                    } else {
                        let _ = Text::with_baseline(name, Point::new(7, y), med, Baseline::Top)
                            .draw(&mut display);
                    }
                }

                // Other box: ABOUT / CONTROLS with a ">" cursor.
                let _ = Rectangle::new(Point::new(74, 11), Size::new(50, 15))
                    .into_styled(stroke)
                    .draw(&mut display);
                for i in ui::PROCESS_COUNT..ui::MAIN_ITEMS.len() {
                    let y = 13 + (i - ui::PROCESS_COUNT) as i32 * 6;
                    let marker = if i == view.cursor { ">" } else { " " };
                    let mut l = FmtBuf::new();
                    let _ = write!(l, "{}{}", marker, ui::MAIN_ITEMS[i]);
                    let _ = Text::with_baseline(l.as_str(), Point::new(76, y), small, Baseline::Top)
                        .draw(&mut display);
                }

                // 5 process-status rects, bottom-right. The IMU block (i=2)
                // blinks while it is ramping up (not ready yet), solid once ready.
                for i in 0..5 {
                    let r = Rectangle::new(Point::new(76 + i as i32 * 9, 44), Size::new(7, 7));
                    let filled = if i == 2 {
                        control::imu_on()
                            && (control::imu_ramp_q8(now) >= 256 || (now / 150) % 2 == 0)
                    } else {
                        control::process_running(i)
                    };
                    if filled {
                        let _ = r.into_styled(fill).draw(&mut display);
                    } else {
                        let _ = r.into_styled(stroke).draw(&mut display);
                    }
                }
            }
        }
        // Ignore flush errors; the next frame retries.
        let _ = display.flush().await;

        // The fluid wants to run as fast as the flush allows; menus don't.
        let frame_ms = if view.screen == ViewScreen::Fluids { 2 } else { 40 };
        Timer::after(Duration::from_millis(frame_ms)).await;
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
