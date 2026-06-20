//! SH1106 128x64 OLED over I2C (SCL=GPIO18, SDA=GPIO23, addr 0x3C).

use embassy_time::{Duration, Timer};
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Baseline, Text};
use esp_hal::i2c::master::{Config, I2c};
use esp_hal::peripherals::{GPIO18, GPIO23, I2C0};
use esp_hal::time::Rate;
use oled_async::builder::Builder;
use oled_async::prelude::*;

#[embassy_executor::task]
pub async fn run(i2c: I2C0<'static>, scl: GPIO18<'static>, sda: GPIO23<'static>) {
    let i2c_bus = I2c::new(i2c, Config::default().with_frequency(Rate::from_khz(400)))
        .unwrap()
        .with_scl(scl)
        .with_sda(sda)
        .into_async();

    let interface = display_interface_i2c::I2CInterface::new(i2c_bus, 0x3C, 0x40);
    let raw_disp = Builder::new(oled_async::displays::sh1106::Sh1106_128_64 {})
        .with_rotation(DisplayRotation::Rotate0)
        .connect(interface);
    let mut display: GraphicsMode<_, _> = raw_disp.into();
    display.init().await.unwrap();
    // SH1106 GDDRAM holds garbage on power-up.
    display.clear();
    display.flush().await.unwrap();

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
        let x = if phase < track { phase } else { 2 * track - phase };

        display.clear();
        Text::with_baseline("Doktorhut", Point::new(0, 0), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        Text::with_baseline("  Flo!", Point::new(0, 12), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        Rectangle::new(Point::new(x, h as i32 - 10), Size::new(box_w as u32, 8))
            .into_styled(box_style)
            .draw(&mut display)
            .unwrap();
        display.flush().await.unwrap();

        frame = frame.wrapping_add(2);
        Timer::after(Duration::from_millis(40)).await;
    }
}
