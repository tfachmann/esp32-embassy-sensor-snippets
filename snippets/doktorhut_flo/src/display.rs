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
use crate::{about, control, fluid, nyancat, tilt3d, ui};

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
        .with_rotation(DisplayRotation::Rotate180)
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
    let small_inv = MonoTextStyleBuilder::new()
        .font(&FONT_4X6)
        .text_color(BinaryColor::Off)
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
        ui::poll(); // auto-enter a pending IMU screen once the IMU is ready
        let view = ui::view();

        display.clear();
        match view.screen {
            ViewScreen::Controls => {
                render_main_menu(&mut display, &view, small, med, med_inv, now);
                let win = Rectangle::new(Point::new(2, 4), Size::new(124, 56));
                let content = draw_window(&mut display, win, "CONTROLS", small, small_inv);
                for (i, name) in ui::CONTROL_ITEMS.iter().enumerate() {
                    let y = content + 2 + i as i32 * 10;
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
                        0 => row_bar(&mut display, text_style, marker, "Vol", 6, y,
                            control::volume(), control::VOLUME_MIN, control::VOLUME_MAX),
                        1 => row_bar(&mut display, text_style, marker, "Spd", 6, y,
                            control::speed(), control::SPEED_MIN, control::SPEED_MAX),
                        2 => row_bar(&mut display, text_style, marker, "Brt", 6, y,
                            control::brightness_level(), control::BRIGHTNESS_MIN, control::BRIGHTNESS_MAX),
                        _ => {
                            let mut l = FmtBuf::new();
                            let _ = write!(l, "{} {}", marker, name);
                            let _ = Text::with_baseline(l.as_str(), Point::new(6, y), text_style, Baseline::Top)
                                .draw(&mut display);
                        }
                    }
                }
            }
            ViewScreen::Fluids => {
                render_main_menu(&mut display, &view, small, med, med_inv, now);
                let win = Rectangle::new(Point::new(4, 4), Size::new(100, 56));
                let content = draw_window(&mut display, win, "FLUIDS", small, small_inv);
                // Region is taller than the 24x10 fluid -> the render centers it,
                // giving a small top (and bottom) padding under the title bar.
                fluid::step_and_render(
                    scene,
                    &mut display,
                    control::accel_x(),
                    control::accel_y(),
                    6,
                    content,
                    96,
                    60 - content,
                );
            }
            ViewScreen::About => {
                render_main_menu(&mut display, &view, small, med, med_inv, now);
                let win = Rectangle::new(Point::new(0, 0), Size::new(128, 64));
                let content = draw_window(&mut display, win, "ABOUT", small, small_inv);
                about::render(&mut display, text_style, small, content);
            }
            ViewScreen::Party => {
                // Hidden easter egg: full-screen nyancat animation.
                let _ = nyancat::frame(now).draw(&mut display);
            }
            ViewScreen::BeerManual => {
                // Normal menu in the background + a small floating dialog on top.
                render_main_menu(&mut display, &view, small, med, med_inv, now);
                let win = Rectangle::new(Point::new(10, 16), Size::new(108, 32));
                let content = draw_window(&mut display, win, "BEER MANUAL", small, small_inv);
                // Inverted bar so it grows the same way the knob turns.
                let inv = control::SERVO_MAX + control::SERVO_MIN - control::servo_pos();
                draw_bar(&mut display, 16, content + 4, 96, 10, inv,
                    control::SERVO_MIN, control::SERVO_MAX);
            }
            ViewScreen::Tilt => {
                // Menu in the background + a floating window holding the gizmo.
                render_main_menu(&mut display, &view, small, med, med_inv, now);
                let win = Rectangle::new(Point::new(4, 8), Size::new(100, 52));
                let content = draw_window(&mut display, win, "TILT", small, small_inv);
                tilt3d::render(
                    &mut display,
                    control::pitch(),
                    control::roll(),
                    control::accel_x(),
                    control::accel_y(),
                    control::accel_z(),
                    small,
                    4,
                    content,
                    100,
                    60 - content,
                );
            }
            ViewScreen::Main => {
                render_main_menu(&mut display, &view, small, med, med_inv, now);
            }
        }
        // Ignore flush errors; the next frame retries.
        let _ = display.flush().await;

        // Fluid runs a bit slower than max to leave the shared timer / cross-core
        // critical section free, so the LED tasks on core1 stay smooth (the sim
        // is real-time regardless via wall-clock substeps). Menus are slow.
        let frame_ms = match view.screen {
            ViewScreen::Party => 20,
            ViewScreen::Fluids => 16,
            _ => 40,
        };
        Timer::after(Duration::from_millis(frame_ms)).await;
    }
}

/// Floating dialog: clear behind it, rounded border, and a filled title bar
/// (the "bigger top border" that marks it as a window). Returns the content-top
/// y (just below the title bar).
fn draw_window<D>(
    display: &mut D,
    win: Rectangle,
    title: &str,
    small: MonoTextStyle<'_, BinaryColor>,
    small_inv: MonoTextStyle<'_, BinaryColor>,
) -> i32
where
    D: DrawTarget<Color = BinaryColor>,
{
    const TITLEBAR: u32 = 9;
    let stroke = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::On)
        .stroke_width(1)
        .build();
    let _ = win
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
        .draw(display);
    let _ = RoundedRectangle::with_equal_corners(win, Size::new(4, 4))
        .into_styled(stroke)
        .draw(display);
    // Filled title bar across the top, with the title in inverted text.
    let bar = Rectangle::new(
        win.top_left + Point::new(1, 1),
        Size::new(win.size.width - 2, TITLEBAR),
    );
    let _ = bar
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display);
    let _ = Text::with_baseline(title, win.top_left + Point::new(3, 2), small_inv, Baseline::Top)
        .draw(display);
    let _ = small; // (kept for symmetry/future use)
    win.top_left.y + 1 + TITLEBAR as i32
}

/// The desktop-window main menu (processes box, other box, status rects).
fn render_main_menu<D>(
    display: &mut D,
    view: &ui::View,
    small: MonoTextStyle<'_, BinaryColor>,
    med: MonoTextStyle<'_, BinaryColor>,
    med_inv: MonoTextStyle<'_, BinaryColor>,
    now: u32,
) where
    D: DrawTarget<Color = BinaryColor>,
{
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
    .draw(display);
    let _ = Text::with_baseline("LN 2.8.2", Point::new(48, 2), small, Baseline::Top).draw(display);

    // Processes box: selected row is inverted (no arrow).
    let _ = Rectangle::new(Point::new(4, 11), Size::new(66, 49))
        .into_styled(stroke)
        .draw(display);
    for i in 0..ui::PROCESS_COUNT {
        let y = 12 + i as i32 * 8;
        let name = ui::MAIN_ITEMS[i];
        if i == view.cursor {
            let _ = Rectangle::new(Point::new(5, y - 1), Size::new(64, 9))
                .into_styled(fill)
                .draw(display);
            let _ = Text::with_baseline(name, Point::new(7, y), med_inv, Baseline::Top).draw(display);
        } else {
            let _ = Text::with_baseline(name, Point::new(7, y), med, Baseline::Top).draw(display);
        }
    }

    // Other box: ABOUT / CONTROLS with a ">" cursor.
    let _ = Rectangle::new(Point::new(74, 11), Size::new(50, 15))
        .into_styled(stroke)
        .draw(display);
    for i in ui::PROCESS_COUNT..ui::MAIN_ITEMS.len() {
        let y = 13 + (i - ui::PROCESS_COUNT) as i32 * 6;
        let marker = if i == view.cursor { ">" } else { " " };
        let mut l = FmtBuf::new();
        let _ = write!(l, "{}{}", marker, ui::MAIN_ITEMS[i]);
        let _ = Text::with_baseline(l.as_str(), Point::new(76, y), small, Baseline::Top).draw(display);
    }

    // Process-status rects, bottom-right. The IMU block blinks while ramping.
    for i in 0..ui::PROCESS_COUNT {
        let r = Rectangle::new(Point::new(76 + i as i32 * 8, 44), Size::new(7, 7));
        let filled = if i == ui::MAIN_IMU {
            control::imu_on() && (control::imu_ramp_q8(now) >= 256 || (now / 150) % 2 == 0)
        } else {
            control::process_running(i)
        };
        if filled {
            let _ = r.into_styled(fill).draw(display);
        } else {
            let _ = r.into_styled(stroke).draw(display);
        }
    }
}

/// A control row: "<marker> <label>" at x=ox, a value bar to its right.
fn row_bar<D>(
    display: &mut D,
    style: MonoTextStyle<'_, BinaryColor>,
    marker: &str,
    label: &str,
    ox: i32,
    y: i32,
    value: u32,
    min: u32,
    max: u32,
) where
    D: DrawTarget<Color = BinaryColor>,
{
    let mut l = FmtBuf::new();
    let _ = write!(l, "{} {}", marker, label);
    let _ = Text::with_baseline(l.as_str(), Point::new(ox, y), style, Baseline::Top).draw(display);
    draw_bar(display, ox + 34, y + 1, 116 - (ox + 34), 7, value, min, max);
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
