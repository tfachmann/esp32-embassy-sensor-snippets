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
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::timer::timg::TimerGroup;
use esp_println::logger::init_logger;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[embassy_executor::task]
async fn blink_led(mut led: Output<'static>) {
    loop {
        led.set_high();
        log::info!("blink");
        Timer::after(Duration::from_millis(200)).await;
        led.set_low();
        Timer::after(Duration::from_millis(800)).await;
    }
}

#[embassy_executor::task]
async fn read_rotary(a: Input<'static>, b: Input<'static>) {
    let mut rotary = rotary_encoder_hal::Rotary::new(a, b);
    let mut counter: i32 = 0;
    loop {
        let (a, b) = rotary.pins();
        embassy_futures::select::select(a.wait_for_any_edge(), b.wait_for_any_edge()).await;
        match rotary.update().unwrap() {
            rotary_encoder_hal::Direction::Clockwise => {
                counter += 1;
                log::info!("counter: {counter}");
            }
            rotary_encoder_hal::Direction::CounterClockwise => {
                counter -= 1;
                log::info!("counter: {counter}");
            }
            rotary_encoder_hal::Direction::None => (),
        }
    }
}

#[embassy_executor::task]
async fn read_rotary_button(mut sw: Input<'static>) {
    loop {
        sw.wait_for_any_edge().await;
        Timer::after(Duration::from_millis(20)).await;
        if !sw.is_high() {
            log::info!("Button Press");
        } else {
            log::info!("Button Release");
        }
    }
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // generator version: 0.5.0
    init_logger(log::LevelFilter::Info);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let timer0 = TimerGroup::new(peripherals.TIMG1);
    esp_hal::interrupt::enable(
        esp_hal::peripherals::Interrupt::GPIO,
        esp_hal::interrupt::Priority::Priority1,
    )
    .unwrap();
    let a_clk = Input::new(
        peripherals.GPIO18,
        InputConfig::default().with_pull(Pull::Up),
    );
    let b_dt = Input::new(
        peripherals.GPIO19,
        InputConfig::default().with_pull(Pull::Up),
    );
    let sw = Input::new(
        peripherals.GPIO21,
        InputConfig::default().with_pull(Pull::Up),
    );
    esp_hal_embassy::init(timer0.timer0);
    let led = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());
    spawner.spawn(blink_led(led)).ok();
    spawner.spawn(read_rotary(a_clk, b_dt)).ok();
    spawner.spawn(read_rotary_button(sw)).ok();

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.0/examples/src/bin
}
