#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println::logger::init_logger;
use log::{error, info};
use ssd1306::mode::DisplayConfigAsync;
use ssd1306::size::DisplaySize128x64;
use ssd1306::{I2CDisplayInterface, Ssd1306Async};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    error!("Program panicked!");
    loop {}
}

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[embassy_executor::task]
async fn blink_led(mut led: Output<'static>) {
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(200)).await;
        led.set_low();
        Timer::after(Duration::from_millis(800)).await;
    }
}

#[embassy_executor::task]
async fn counter() {
    let mut cnt: u8 = 0;
    loop {
        info!("I am counting... {cnt}");
        Timer::after(Duration::from_millis(500)).await;
        cnt = cnt.wrapping_add(1);
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // generator version: 0.5.0
    esp_alloc::heap_allocator!(size: 64 * 1024);
    init_logger(log::LevelFilter::Info);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timer0 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    let led = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

    let i2c_bus = esp_hal::i2c::master::I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_scl(peripherals.GPIO18)
    .with_sda(peripherals.GPIO23)
    .into_async();

    let interface = I2CDisplayInterface::new(i2c_bus);
    let mut display = Ssd1306Async::new(
        interface,
        DisplaySize128x64,
        ssd1306::prelude::DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();
    display.init().await.unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    Text::with_baseline(
        "Hello, Rust...\n  ...to ssd1306!",
        Point::new(0, 16),
        text_style,
        embedded_graphics::text::Baseline::Top,
    )
    .draw(&mut display)
    .unwrap();

    display.flush().await.unwrap();

    spawner.spawn(blink_led(led)).ok();
    spawner.spawn(counter()).ok();
}
