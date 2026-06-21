# I2C Sharing

Two I2C devices on a single bus, shared between two embassy tasks:

In this example:
- **SH1106** OLED display (`0x3C`)
- **MPU6050** IMU (`0x68`)

The bus lives in an `embassy_sync::mutex::Mutex` (held in a `StaticCell` so it
is `'static`). Each task gets its own `I2cDevice` handle from
`embassy-embedded-hal`; the mutex serializes access so the two tasks never
collide on the wire.
