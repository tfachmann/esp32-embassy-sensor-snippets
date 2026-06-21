//! Shared I2C bus: one mutex-guarded bus, an `I2cDevice` handle per task.

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use esp_hal::i2c::master::I2c;
use esp_hal::Async;

pub type SharedBus = Mutex<CriticalSectionRawMutex, I2c<'static, Async>>;
pub type SharedI2c = I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Async>>;
