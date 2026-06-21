#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::fmt::Write;
use core::sync::atomic::{AtomicI32, Ordering::Relaxed};

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::I2c;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{i2c, Async};
use esp_println::logger::init_logger;
use oled_async::builder::Builder;
use oled_async::prelude::*;
use static_cell::StaticCell;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    esp_println::println!("PANIC: {info}");
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

// One I2C bus, wrapped in a mutex; each task gets its own I2cDevice handle.
type SharedBus = Mutex<CriticalSectionRawMutex, I2c<'static, Async>>;
type SharedI2c = I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>;
static I2C_BUS: StaticCell<SharedBus> = StaticCell::new();

// From the MPU task to the display task:
// orientation (deg, accel-derived) + angular rate (deg/s, gyro).
static PITCH: AtomicI32 = AtomicI32::new(0);
static ROLL: AtomicI32 = AtomicI32::new(0);
static GYRO_X: AtomicI32 = AtomicI32::new(0);
static GYRO_Y: AtomicI32 = AtomicI32::new(0);
static GYRO_Z: AtomicI32 = AtomicI32::new(0);

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
async fn display_task(i2c: SharedI2c) {
    let interface = display_interface_i2c::I2CInterface::new(i2c, 0x3C, 0x40);
    let raw_disp = Builder::new(oled_async::displays::sh1106::Sh1106_128_64 {})
        .with_rotation(DisplayRotation::Rotate0)
        .connect(interface);
    let mut display: GraphicsMode<_, _> = raw_disp.into();
    display.init().await.unwrap();
    // SH1106 GDDRAM holds garbage on power-up.
    display.clear();
    display.flush().await.unwrap();

    let style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    loop {
        let mut angle = FmtBuf::new();
        let _ = write!(angle, "P:{} R:{}", PITCH.load(Relaxed), ROLL.load(Relaxed));
        let mut rate = FmtBuf::new();
        let _ = write!(
            rate,
            "X:{} Y:{} Z:{}",
            GYRO_X.load(Relaxed),
            GYRO_Y.load(Relaxed),
            GYRO_Z.load(Relaxed)
        );

        display.clear();
        Text::with_baseline("Angle (deg)", Point::new(0, 0), style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        Text::with_baseline(angle.as_str(), Point::new(0, 12), style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        Text::with_baseline("Rate (dps)", Point::new(0, 30), style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        Text::with_baseline(rate.as_str(), Point::new(0, 42), style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        display.flush().await.unwrap();

        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task]
async fn mpu_task(i2c: SharedI2c) {
    // Address 0x68 (AD0 low).
    let mut mpu = edrv_mpu6050::MPU6050::new(i2c, 0x68);

    Timer::after(Duration::from_millis(100)).await;

    // This module is a clone (WHO_AM_I 0x72), so configure registers directly
    // instead of the driver's init(). Ranges match MPU6050::new() defaults.
    mpu.write_reg(0x6B, 0x00).await.unwrap(); // PWR_MGMT_1: wake from sleep
    mpu.write_reg(0x1A, 0x01).await.unwrap(); // CONFIG: DLPF 184 Hz
    mpu.write_reg(0x1B, 0x10).await.unwrap(); // GYRO_CONFIG: +-1000 deg/s
    mpu.write_reg(0x1C, 0x00).await.unwrap(); // ACCEL_CONFIG: +-2g
    log::info!("mpu initialized");

    loop {
        // orientation from gravity (pitch/roll); yaw needs a magnetometer
        if let Ok((ax, ay, az)) = mpu.read_accel().await {
            let pitch = libm::atan2f(-ax, libm::sqrtf(ay * ay + az * az)).to_degrees();
            let roll = libm::atan2f(ay, az).to_degrees();
            PITCH.store(pitch as i32, Relaxed);
            ROLL.store(roll as i32, Relaxed);
        }
        // angular rate from the gyro
        if let Ok((gx, gy, gz)) = mpu.read_gyro().await {
            GYRO_X.store(gx as i32, Relaxed);
            GYRO_Y.store(gy as i32, Relaxed);
            GYRO_Z.store(gz as i32, Relaxed);
            log::info!("gyro[deg/s] x={gx:.1} y={gy:.1} z={gz:.1}");
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
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

    // Share the single bus between both device handles.
    let bus = I2C_BUS.init(Mutex::new(i2c0));
    spawner.spawn(display_task(I2cDevice::new(bus))).ok();
    spawner.spawn(mpu_task(I2cDevice::new(bus))).ok();
}
