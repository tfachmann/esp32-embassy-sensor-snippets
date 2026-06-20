#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::rmt::{PulseCode, Rmt, TxChannelAsync, TxChannelConfig, TxChannelCreator};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println::logger::init_logger;
use log::{error, info};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    error!("Program panicked!");
    loop {}
}

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

/// Number of WS2812B LEDs on the strip.
const NUM_LEDS: usize = 8;

// RMT src clock is configured to 80 MHz with a clock divider of 1, so one tick = 12.5 ns.
const T0H: u16 = 32; // 0.40 us
const T0L: u16 = 68; // 0.85 us
const T1H: u16 = 64; // 0.80 us
const T1L: u16 = 36; // 0.45 us

// 24 bits per LED, plus one terminating empty code that drops the line to idle low.
const BUF_LEN: usize = NUM_LEDS * 24 + 1;

/// Encode RGB colors into the RMT pulse-code buffer.
///
/// WS2812B expects the bits in **GRB** order, most-significant bit first.
fn encode(colors: &[[u8; 3]; NUM_LEDS], buf: &mut [u32; BUF_LEN]) {
    let mut i = 0;
    for &[r, g, b] in colors {
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
    buf[i] = PulseCode::empty(); // end marker -> line returns to idle low
}

/// Map a 0..=255 position to an RGB color cycling through the color wheel.
fn wheel(mut pos: u8) -> [u8; 3] {
    pos = 255 - pos;
    if pos < 85 {
        [255 - pos * 3, 0, pos * 3]
    } else if pos < 170 {
        pos -= 85;
        [0, pos * 3, 255 - pos * 3]
    } else {
        pos -= 170;
        [pos * 3, 255 - pos * 3, 0]
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // generator version: 0.5.0
    init_logger(log::LevelFilter::Info);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timer0 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    // "I am alive" onboard LED (GPIO2) + serial counter
    let led = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());
    spawner.spawn(blink_led(led)).ok();
    spawner.spawn(counter()).ok();

    // RMT at 80 MHz -> 12.5 ns per tick (see the T0H/T1H constants above).
    let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80))
        .unwrap()
        .into_async();

    // Data line of the strip. GPIO5 is a plain output with no boot-strapping
    // duties on the ESP32 -- change it to match your wiring.
    let mut channel = rmt
        .channel0
        .configure_tx(
            peripherals.GPIO5,
            TxChannelConfig::default()
                .with_clk_divider(1)
                .with_idle_output(true)
                .with_idle_output_level(Level::Low)
                .with_memsize(4),
        )
        .unwrap();

    info!("Driving {NUM_LEDS} WS2812B LEDs over RMT (GPIO5)");

    let mut buf = [PulseCode::empty(); BUF_LEN];
    let mut colors = [[0u8; 3]; NUM_LEDS];
    let mut frame: u8 = 0;

    loop {
        for (i, c) in colors.iter_mut().enumerate() {
            // spread the wheel across the strip, then shift it each frame
            let pos = ((i * 256 / NUM_LEDS) as u8).wrapping_add(frame);
            let mut rgb = wheel(pos);
            // dim to ~1/8 brightness -- full brightness is blinding and draws
            // a lot of current (each LED can pull ~60 mA at full white).
            for v in rgb.iter_mut() {
                *v /= 8;
            }
            *c = rgb;
        }

        encode(&colors, &mut buf);
        channel.transmit(&buf).await.unwrap();

        frame = frame.wrapping_add(1);
        // doubles as the >50 us reset latch between frames
        Timer::after(Duration::from_millis(20)).await;
    }
}
