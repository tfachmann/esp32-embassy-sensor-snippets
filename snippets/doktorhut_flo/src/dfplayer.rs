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

    // MUSIC is a process toggle (off at boot): play/resume when on, pause when
    // off. Also sync volume from `control`.
    let mut applied_vol = u8::MAX;
    let mut playing = false; // current player state
    let mut started = false; // has play(TRACK) been issued at least once
    loop {
        let want = control::music_on();
        if want != playing {
            let r = if want {
                if started {
                    player.resume().await
                } else {
                    started = true;
                    player.play(TRACK).await
                }
            } else {
                player.pause().await
            };
            if r.is_ok() {
                playing = want;
            }
        }

        let vol = control::volume() as u8;
        if vol != applied_vol && player.set_volume(vol).await.is_ok() {
            applied_vol = vol;
        }

        Timer::after(Duration::from_millis(200)).await;
    }
}
