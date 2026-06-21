//! MPU6050 IMU on the shared I2C bus. Publishes accel-derived orientation
//! (pitch/roll) to `control` for the display. Address 0x68.

use embassy_time::{Duration, Timer};

use crate::bus::SharedI2c;
use crate::control;

#[embassy_executor::task]
pub async fn run(i2c: SharedI2c) {
    let mut mpu = edrv_mpu6050::MPU6050::new(i2c, 0x68);

    // Let the MPU power rail settle before the first transaction.
    Timer::after(Duration::from_millis(300)).await;

    // Clone module (WHO_AM_I 0x72): configure registers directly instead of the
    // driver's init(). On a cold boot the chip can latch into a bad state, so
    // start every attempt with a DEVICE_RESET to force a known state, then wake
    // and configure. Retry rather than unwrap -- a NACK must not panic (that
    // would freeze everything). Ranges match MPU6050::new() defaults.
    let mut configured = false;
    for _ in 0..20 {
        // PWR_MGMT_1 DEVICE_RESET, then let the reset complete.
        if mpu.write_reg(0x6B, 0x80).await.is_ok() {
            Timer::after(Duration::from_millis(100)).await;
            // && short-circuits on the first failing write.
            let ok = mpu.write_reg(0x6B, 0x00).await.is_ok() // PWR_MGMT_1: wake
                && mpu.write_reg(0x1A, 0x01).await.is_ok() // CONFIG: DLPF 184 Hz
                && mpu.write_reg(0x1B, 0x10).await.is_ok() // GYRO_CONFIG: +-1000 deg/s
                && mpu.write_reg(0x1C, 0x00).await.is_ok(); // ACCEL_CONFIG: +-2g
            if ok {
                configured = true;
                break;
            }
        }
        log::warn!("imu init failed, retrying...");
        Timer::after(Duration::from_millis(100)).await;
    }
    if !configured {
        log::error!("imu init gave up; continuing without IMU");
        return;
    }
    log::info!("imu initialized");

    loop {
        if let Ok((ax, ay, az)) = mpu.read_accel().await {
            let pitch = libm::atan2f(-ax, libm::sqrtf(ay * ay + az * az)).to_degrees();
            let roll = libm::atan2f(ay, az).to_degrees();
            control::set_orientation(pitch as i32, roll as i32);
            control::set_accel(ax, ay, az);
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}
