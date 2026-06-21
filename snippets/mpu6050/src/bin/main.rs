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
fn panic(info: &core::panic::PanicInfo) -> ! {
    esp_println::println!("PANIC: {info}");
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
async fn read_mpu6050(i2c: I2c<'static, Async>) {
    // I2C address: 0x68 (AD0 low), 0x69 (AD0 high). See README.
    const ADDR: u8 = 0x68;
    let mut mpu = edrv_mpu6050::MPU6050::new(i2c, ADDR);

    // The MPU6050 needs a moment after power-on before it ACKs.
    Timer::after(Duration::from_millis(100)).await;

    match mpu.read_reg(0x75).await {
        Ok(id) => log::info!("WHO_AM_I = 0x{id:02X}"),
        Err(_) => log::warn!("WHO_AM_I read failed (bus NACK)"),
    }

    // This module is a clone reporting WHO_AM_I 0x72, so the driver's init()
    // rejects it. Configure the registers directly — clones are register
    // compatible. Ranges must match MPU6050::new() defaults (gyro +-1000 deg/s,
    // accel +-2g) so read_accel/read_gyro scale correctly.
    mpu.write_reg(0x6B, 0x00).await.unwrap(); // PWR_MGMT_1: wake from sleep
    mpu.write_reg(0x1A, 0x01).await.unwrap(); // CONFIG: DLPF 184 Hz
    mpu.write_reg(0x1B, 0x10).await.unwrap(); // GYRO_CONFIG: +-1000 deg/s
    mpu.write_reg(0x1C, 0x00).await.unwrap(); // ACCEL_CONFIG: +-2g
    log::info!("mpu6050 initialized");

    loop {
        // accel in g, gyro in deg/s (rotation rate)
        let Ok((ax, ay, az)) = mpu.read_accel().await else {
            continue;
        };
        let Ok((gx, gy, gz)) = mpu.read_gyro().await else {
            continue;
        };
        log::info!("accel[g] x={ax:.2} y={ay:.2} z={az:.2}  gyro[deg/s] x={gx:.1} y={gy:.1} z={gz:.1}");
        Timer::after(Duration::from_millis(100)).await;
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
        i2c::master::Config::default().with_frequency(Rate::from_khz(100)),
    )
    .unwrap()
    .with_scl(peripherals.GPIO18)
    .with_sda(peripherals.GPIO23)
    .into_async();

    let led = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

    spawner.spawn(blink_led(led)).ok();
    spawner.spawn(read_mpu6050(i2c0)).ok();
}
