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
use esp_hal::i2c::master::I2c;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{i2c, Async};
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
async fn read_gy_271(i2c: I2c<'static, Async>) {
    // For the address, refer to the README
    const ADDR: u8 = 0x1E;
    // 0x1E => HMC5883L
    // 0x0D => QMC5883L
    let mut hmc5883l = edrv_hmc5883l::HMC5883L::new(i2c, ADDR);
    let config = edrv_hmc5883l::Config::default();
    hmc5883l.init(config).await.unwrap();

    loop {
        let Ok((x, y, z)) = hmc5883l.read_measurement().await else {
            continue;
        };
        log::info!("x={x:.3}  y={y:.3}  z={z:.3}");
        Timer::after(Duration::from_millis(50)).await;
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
    esp_hal_embassy::init(timer0.timer0);

    let i2c0 = i2c::master::I2c::new(
        peripherals.I2C0,
        i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_scl(peripherals.GPIO18)
    .with_sda(peripherals.GPIO23)
    .into_async();

    let led = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

    spawner.spawn(blink_led(led)).ok();
    spawner.spawn(read_gy_271(i2c0)).ok();
}
