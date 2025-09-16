#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use bmp180_embedded_hal::asynch::UninitBMP180;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay, Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::I2c;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::Async;
use esp_println::logger::init_logger;

// static I2C_BUS: StaticCell<Mutex<NoopRawMutex, I2c<'_, Async>>> = StaticCell::new();

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
async fn read_bmp180(i2c: I2c<'static, Async>) {
    let i2c_bus: Mutex<NoopRawMutex, _> = Mutex::new(i2c);
    let i2c_dev1 = I2cDevice::new(&i2c_bus);
    let mut bmp180 = UninitBMP180::builder(i2c_dev1, Delay {})
        .mode(bmp180_embedded_hal::Mode::UltraHighResolution)
        .build()
        .initialize()
        .await
        .unwrap();
    let calibration = bmp180.calibration();
    log::info!("{calibration:?}");

    loop {
        bmp180.update().await.ok();
        let temp = bmp180.temperature_celsius();
        log::info!("temp: {temp} *C");
        let pressure = bmp180.pressure();
        log::info!("pressure: {pressure} Pa");
        Timer::after(Duration::from_millis(500)).await
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

    // let i2c0 = I2C::new(
    //     peripherals.I2C0,
    //     io.pins.gpio21, // SDA
    //     io.pins.gpio22, // SCL
    //     400.kHz(),
    //     &clocks,
    // );

    let i2c0 = esp_hal::i2c::master::I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_scl(peripherals.GPIO18)
    .with_sda(peripherals.GPIO23)
    .into_async();

    let led = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

    spawner.spawn(blink_led(led)).ok();
    spawner.spawn(read_bmp180(i2c0)).ok();

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.0/examples/src/bin
}
