#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::analog::adc::{Adc, AdcConfig, Attenuation};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::peripherals::ADC2;
use esp_hal::peripherals::GPIO13;
use esp_hal::peripherals::GPIO14;
use esp_hal::timer::timg::TimerGroup;
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
        Timer::after(Duration::from_millis(200)).await;
        led.set_low();
        Timer::after(Duration::from_millis(800)).await;
    }
}

#[embassy_executor::task]
async fn read_joystick(
    vrx_pin: GPIO13<'static>,
    vry_pin: GPIO14<'static>,
    adc_pin: ADC2<'static>,
    btn: Input<'static>,
) {
    let mut adc_config = AdcConfig::new();
    let mut vrx_pin = adc_config.enable_pin(vrx_pin, Attenuation::_11dB);
    let mut vry_pin = adc_config.enable_pin(vry_pin, Attenuation::_11dB);
    let mut adc = Adc::new(adc_pin, adc_config);

    let mut prev_vrx: u16 = 0;
    let mut prev_vry: u16 = 0;
    let mut prev_btn_state = false;
    let mut print_vals = true;

    loop {
        let Ok(vry) = nb::block!(adc.read_oneshot(&mut vry_pin)) else {
            continue;
        };
        let Ok(vrx) = nb::block!(adc.read_oneshot(&mut vrx_pin)) else {
            continue;
        };
        if vrx.abs_diff(prev_vrx) > 20 {
            prev_vrx = vrx;
            print_vals = true;
        }
        if vry.abs_diff(prev_vry) > 20 {
            prev_vry = vry;
            print_vals = true;
        }

        let btn_state = btn.is_low();
        if btn_state && !prev_btn_state {
            log::info!("Button Pressed");
            print_vals = true;
        }
        prev_btn_state = btn_state;

        if print_vals {
            print_vals = false;
            log::info!("X: {vrx} Y: {vry}\r\n");
        }

        Timer::after(Duration::from_millis(10)).await;
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

    let vrx_pin = peripherals.GPIO13;
    let vry_pin = peripherals.GPIO14;
    let adc_pin = peripherals.ADC2;
    let btn = Input::new(
        peripherals.GPIO32,
        InputConfig::default().with_pull(Pull::Up),
    );

    let led = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

    spawner.spawn(blink_led(led)).ok();
    spawner
        .spawn(read_joystick(vrx_pin, vry_pin, adc_pin, btn))
        .ok();

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.0/examples/src/bin
}
