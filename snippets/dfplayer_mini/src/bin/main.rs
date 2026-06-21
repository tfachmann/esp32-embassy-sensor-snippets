#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use dfplayer_async::DfPlayer;
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::{self};
use esp_hal::Async;
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
        Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        Timer::after(Duration::from_millis(400)).await;
    }
}

#[embassy_executor::task]
async fn play_mp3(mut uart: uart::Uart<'static, Async>, mut led: Output<'static>) {
    led.set_low();
    for _ in 0..2 {
        led.set_high();
        Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        Timer::after(Duration::from_millis(200)).await;
    }
    log::info!("Initializing dfplayer...");
    let mut dfplayer = DfPlayer::new(&mut uart, true, 1_000, TimeSrc, embassy_time::Delay, None)
        .await
        .inspect_err(|e| log::error!("{e:?}"))
        .unwrap();
    for _ in 0..3 {
        led.set_high();
        Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        Timer::after(Duration::from_millis(200)).await;
    }
    // match dfplayer.query_status().await {
    //     Ok(_) => led.set_high(),
    //     _ => panic!(),
    // }

    log::info!("setting volume");
    dfplayer.set_volume(10).await.unwrap();
    log::info!("playing");
    let song = 1;
    dfplayer.play(song).await.unwrap();

    loop {
        Timer::after(Duration::from_millis(200)).await;
    }

    // loop {
    //     Timer::after(Duration::from_millis(100)).await;
    //     log::info!("waiting for busy pin");
    //     busy_pin.wait_for_high().await;
    //     let _ = dfplayer
    //         .play(song)
    //         .await
    //         .inspect_err(|e| log::error!("{e:?}"));
    //     // .unwrap();
    // }
}

struct TimeSrc;
impl dfplayer_async::TimeSource for TimeSrc {
    type Instant = Instant;

    fn now(&self) -> Self::Instant {
        Instant::now()
    }

    fn is_elapsed(&self, since: Self::Instant, timeout_ms: u64) -> bool {
        Instant::now().duration_since(since) >= Duration::from_millis(timeout_ms)
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

    let uart_config = uart::Config::default().with_baudrate(9600);
    let uart = uart::Uart::new(peripherals.UART1, uart_config)
        .unwrap()
        .with_rx(peripherals.GPIO16)
        .with_tx(peripherals.GPIO17)
        .into_async();
    let mut led = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());
    for _ in 0..2 {
        led.set_high();
        Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        Timer::after(Duration::from_millis(200)).await;
    }
    // let busy_pin = Input::new(
    //     peripherals.GPIO27,
    //     InputConfig::default().with_pull(esp_hal::gpio::Pull::None),
    // );

    // spawner.spawn(blink_led(led)).ok();
    spawner.spawn(play_mp3(uart, led)).ok();

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.0/examples/src/bin
}
