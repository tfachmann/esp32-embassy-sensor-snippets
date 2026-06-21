//! MPU6050 IMU on the shared I2C bus. Publishes accel-derived orientation
//! (pitch/roll) to `control` for the display. Address 0x68.

use embassy_time::{Duration, Timer};

use crate::bus::SharedI2c;
use crate::control;

#[embassy_executor::task]
pub async fn run(i2c: SharedI2c) {
    let mut mpu = edrv_mpu6050::MPU6050::new(i2c, 0x68);

    Timer::after(Duration::from_millis(100)).await;

    // Clone module (WHO_AM_I 0x72): configure registers directly instead of the
    // driver's init(). Ranges match MPU6050::new() defaults.
    mpu.write_reg(0x6B, 0x00).await.unwrap(); // PWR_MGMT_1: wake from sleep
    mpu.write_reg(0x1A, 0x01).await.unwrap(); // CONFIG: DLPF 184 Hz
    mpu.write_reg(0x1B, 0x10).await.unwrap(); // GYRO_CONFIG: +-1000 deg/s
    mpu.write_reg(0x1C, 0x00).await.unwrap(); // ACCEL_CONFIG: +-2g

    loop {
        if let Ok((ax, ay, az)) = mpu.read_accel().await {
            let pitch = libm::atan2f(-ax, libm::sqrtf(ay * ay + az * az)).to_degrees();
            let roll = libm::atan2f(ay, az).to_degrees();
            control::set_orientation(pitch as i32, roll as i32);
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}
