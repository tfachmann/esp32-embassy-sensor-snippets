#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use doktorhut_flo::bus::SharedBus;
use doktorhut_flo::{dfplayer, display, imu, led_strip, rotary};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println::logger::init_logger;
use static_cell::StaticCell;
use log::{error, info};

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!("PANIC: {info}");
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[embassy_executor::task]
async fn blink_led(mut led: Output<'static>) {
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(200)).await;
        led.set_low();
        Timer::after(Duration::from_millis(800)).await;
    }
}

#[embassy_executor::task]
async fn counter() {
    let mut cnt: u8 = 0;
    loop {
        info!("I am counting... {cnt}");
        Timer::after(Duration::from_millis(500)).await;
        cnt = cnt.wrapping_add(1);
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    init_logger(log::LevelFilter::Info);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timer0 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    esp_hal::interrupt::enable(
        esp_hal::peripherals::Interrupt::GPIO,
        esp_hal::interrupt::Priority::Priority1,
    )
    .unwrap();

    let led = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());
    spawner.spawn(blink_led(led)).ok();
    spawner.spawn(counter()).ok();

    let pull_up = InputConfig::default().with_pull(Pull::Up);
    let enc_a = Input::new(peripherals.GPIO15, pull_up);
    let enc_b = Input::new(peripherals.GPIO19, pull_up);
    let enc_sw = Input::new(peripherals.GPIO4, pull_up);
    spawner.spawn(rotary::read_encoder(enc_a, enc_b)).ok();
    spawner.spawn(rotary::read_button(enc_sw)).ok();

    // One shared I2C bus for the OLED (0x3C) and the MPU6050 (0x68).
    let i2c = esp_hal::i2c::master::I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_scl(peripherals.GPIO18)
    .with_sda(peripherals.GPIO23)
    .into_async();

    static I2C_BUS: StaticCell<SharedBus> = StaticCell::new();
    let bus = I2C_BUS.init(Mutex::new(i2c));
    spawner.spawn(display::run(I2cDevice::new(bus))).ok();
    spawner.spawn(imu::run(I2cDevice::new(bus))).ok();

    // DFPlayer Mini on UART1 (TX=GPIO17 -> DFPlayer RX, RX=GPIO16 <- DFPlayer TX).
    let uart = esp_hal::uart::Uart::new(
        peripherals.UART1,
        esp_hal::uart::Config::default().with_baudrate(9600),
    )
    .unwrap()
    .with_rx(peripherals.GPIO16)
    .with_tx(peripherals.GPIO17)
    .into_async();
    spawner.spawn(dfplayer::run(uart)).ok();

    let rmt = led_strip::new_rmt(peripherals.RMT);
    let strip0 = led_strip::Ws2812::new(rmt.channel0, peripherals.GPIO25);
    let strip1 = led_strip::Ws2812::new(rmt.channel2, peripherals.GPIO32);
    let strip2 = led_strip::Ws2812::new(rmt.channel4, peripherals.GPIO33);
    spawner.spawn(led_strip::run(strip0)).ok();
    spawner.spawn(led_strip::run(strip1)).ok();
    spawner.spawn(led_strip::run(strip2)).ok();
}
