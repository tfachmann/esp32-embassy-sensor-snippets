//! Rotary encoder input: CLK=GPIO15, DT=GPIO19, SW=GPIO4 (all pull-up).
//! Feeds the menu UI: left/right navigate or edit, click activates.

use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::Input;
use rotary_encoder_hal::{Direction, Rotary};

use crate::ui::{self, Event};

const HOLD_MS: u64 = 3000; // press this long -> Event::Hold (easter egg)

#[embassy_executor::task]
pub async fn read_encoder(a: Input<'static>, b: Input<'static>) {
    let mut rotary = Rotary::new(a, b);
    loop {
        let (a, b) = rotary.pins();
        select(a.wait_for_any_edge(), b.wait_for_any_edge()).await;
        match rotary.update().unwrap() {
            Direction::CounterClockwise => ui::on_input(Event::Right),
            Direction::Clockwise => ui::on_input(Event::Left),
            Direction::None => {}
        }
    }
}

#[embassy_executor::task]
pub async fn read_button(mut sw: Input<'static>) {
    loop {
        sw.wait_for_falling_edge().await; // press (active-low)
        Timer::after(Duration::from_millis(20)).await; // debounce
        if sw.is_high() {
            continue; // bounce/noise, not a real press
        }
        // Race release vs a 3s hold: short -> Click (on release), long -> Hold.
        match select(
            sw.wait_for_rising_edge(),
            Timer::after(Duration::from_millis(HOLD_MS)),
        )
        .await
        {
            Either::First(_) => ui::on_input(Event::Click),
            Either::Second(_) => {
                ui::on_input(Event::Hold);
                sw.wait_for_rising_edge().await; // swallow the eventual release
            }
        }
    }
}
