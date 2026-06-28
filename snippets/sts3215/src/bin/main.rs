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
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::{self, UartTx};
use esp_hal::Async;
use esp_println::logger::init_logger;
use log::{error, info};

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!("PANIC: {info}");
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

// FeeTech STS/SCS serial protocol (half-duplex 1-wire; we only transmit, so a
// WRITE-only TX path is enough to drive the servo to a position).
const SERVO_ID: u8 = 1;
const INST_WRITE: u8 = 0x03;
const ADDR_GOAL_POSITION: u8 = 0x2A; // register 42, 2 bytes little-endian (0..4095)

/// Build a WRITE-goal-position packet into `buf`, returns its length.
///
/// Frame: 0xFF 0xFF ID LEN INSTR ADDR POS_L POS_H CHECKSUM
/// LEN = param_count + 2; CHECKSUM = ~(ID+LEN+INSTR+params) & 0xFF.
fn goal_position_packet(buf: &mut [u8; 9], id: u8, pos: u16) -> usize {
    let pos_l = (pos & 0xFF) as u8;
    let pos_h = (pos >> 8) as u8;
    let len = 5; // ADDR + POS_L + POS_H (3 params) + 2
    buf[0] = 0xFF;
    buf[1] = 0xFF;
    buf[2] = id;
    buf[3] = len;
    buf[4] = INST_WRITE;
    buf[5] = ADDR_GOAL_POSITION;
    buf[6] = pos_l;
    buf[7] = pos_h;
    let sum = id
        .wrapping_add(len)
        .wrapping_add(INST_WRITE)
        .wrapping_add(ADDR_GOAL_POSITION)
        .wrapping_add(pos_l)
        .wrapping_add(pos_h);
    buf[8] = !sum;
    9
}

async fn drive_to(tx: &mut UartTx<'static, Async>, pos: u16) {
    let mut buf = [0u8; 9];
    let n = goal_position_packet(&mut buf, SERVO_ID, pos);
    if let Err(e) = tx.write_async(&buf[..n]).await {
        error!("uart write: {e:?}");
    }
    let _ = tx.flush_async().await;
    info!("goal position -> {pos}");
}

#[embassy_executor::task]
async fn blink_led(mut led: Output<'static>) {
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(200)).await;
        led.set_low();
        Timer::after(Duration::from_millis(800)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    init_logger(log::LevelFilter::Info);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let timer0 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    let led = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());
    spawner.spawn(blink_led(led)).ok();

    // STS3215 data line on GPIO14, UART2, 1 Mbps. TX only (WRITE has no reply).
    let uart_config = uart::Config::default().with_baudrate(1_000_000);
    let mut tx = UartTx::new(peripherals.UART2, uart_config)
        .unwrap()
        .with_tx(peripherals.GPIO14)
        .into_async();

    info!("STS3215: driving between two preset positions");

    // Pre-programmed positions (0..4095 = 0..360 deg). 1024 ~ 90 deg, 3072 ~ 270.
    let presets: [u16; 2] = [1024, 3072];
    let mut i = 0;
    loop {
        drive_to(&mut tx, presets[i]).await;
        i = (i + 1) % presets.len();
        Timer::after(Duration::from_millis(1500)).await;
    }
}
