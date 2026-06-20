//! Rotary encoder input: CLK=GPIO19, DT=GPIO21, SW=GPIO22 (all pull-up).
//! Left = slower, right = faster, click = pause toggle.

use embassy_futures::select::select;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::Input;
use rotary_encoder_hal::{Direction, Rotary};

use crate::control;

#[embassy_executor::task]
pub async fn read_encoder(a: Input<'static>, b: Input<'static>) {
    let mut rotary = Rotary::new(a, b);
    loop {
        let (a, b) = rotary.pins();
        select(a.wait_for_any_edge(), b.wait_for_any_edge()).await;
        match rotary.update().unwrap() {
            Direction::CounterClockwise => control::slower(),
            Direction::Clockwise => control::faster(),
            Direction::None => {}
        }
    }
}

#[embassy_executor::task]
pub async fn read_button(mut sw: Input<'static>) {
    loop {
        sw.wait_for_any_edge().await;
        Timer::after(Duration::from_millis(20)).await; // debounce
        if !sw.is_high() {
            control::toggle_pause();
        }
    }
}
