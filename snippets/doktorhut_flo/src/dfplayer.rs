//! DFPlayer Mini over UART1 (TX=GPIO17, RX=GPIO16, 9600 baud).
//! Inits the module, sets volume, plays a track. Fails gracefully (the task
//! just returns) so the rest of doktorhut keeps running without the player.

use dfplayer_async::{DfPlayer, TimeSource};
use embassy_time::{Duration, Instant, Timer};
use esp_hal::uart::Uart;
use esp_hal::Async;

use crate::control;

const TRACK: u16 = 1;

struct TimeSrc;

impl TimeSource for TimeSrc {
    type Instant = Instant;

    fn now(&self) -> Self::Instant {
        Instant::now()
    }

    fn is_elapsed(&self, since: Self::Instant, timeout_ms: u64) -> bool {
        Instant::now().duration_since(since) >= Duration::from_millis(timeout_ms)
    }
}

#[embassy_executor::task]
pub async fn run(mut uart: Uart<'static, Async>) {
    let mut player =
        match DfPlayer::new(&mut uart, true, 1_000, TimeSrc, embassy_time::Delay, None).await {
            Ok(p) => p,
            Err(e) => {
                log::error!("dfplayer init failed: {e:?}");
                return;
            }
        };
    log::info!("dfplayer initialized");

    if let Err(e) = player.play(TRACK).await {
        log::error!("dfplayer play: {e:?}");
    }

    // Apply volume from `control` whenever it changes (menu edits it).
    let mut applied = u8::MAX;
    loop {
        let want = control::volume() as u8;
        if want != applied && player.set_volume(want).await.is_ok() {
            applied = want;
        }
        Timer::after(Duration::from_millis(200)).await;
    }
}
