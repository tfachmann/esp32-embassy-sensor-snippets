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
use esp_hal::i2c;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println::logger::init_logger;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    // generator version: 0.5.0
    init_logger(log::LevelFilter::Info);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let timer0 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    let mut i2c0 = i2c::master::I2c::new(
        peripherals.I2C0,
        i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_scl(peripherals.GPIO18)
    .with_sda(peripherals.GPIO23)
    .into_async();

    log::info!("Starting I2C scan...");

    for addr in 0x08..0x78 {
        let mut buf = [0u8; 1];
        let res: Result<(), _> = i2c0.read(addr, &mut buf);
        if res.is_ok() {
            log::info!("Found device at 0x{addr:02X}");
        }
        Timer::after(Duration::from_millis(10)).await;
    }

    log::info!("Scan complete.");
}
