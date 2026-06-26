//! FeeTech STS3215 servo on UART2 / GPIO14 (1 Mbps, half-duplex, TX only --
//! a WRITE has no reply). On a BEER trigger it runs a fixed position sequence.

use embassy_time::{Duration, Timer};
use esp_hal::uart::UartTx;
use esp_hal::Async;

use crate::control;

const SERVO_ID: u8 = 1;
const INST_WRITE: u8 = 0x03;
const ADDR_GOAL_POSITION: u8 = 0x2A; // register 42, 2 bytes little-endian (0..4095)

// BEER pour sequence (position, then hold this long before the next).
const SEQUENCE: [u16; 3] = [2000, 1180, 3072];
const STEP_MS: u64 = 2000;

/// Frame: 0xFF 0xFF ID LEN INSTR ADDR POS_L POS_H CHECKSUM.
fn goal_position_packet(id: u8, pos: u16) -> [u8; 9] {
    let pos_l = (pos & 0xFF) as u8;
    let pos_h = (pos >> 8) as u8;
    let len = 5; // 3 params + 2
    let sum = id
        .wrapping_add(len)
        .wrapping_add(INST_WRITE)
        .wrapping_add(ADDR_GOAL_POSITION)
        .wrapping_add(pos_l)
        .wrapping_add(pos_h);
    [
        0xFF,
        0xFF,
        id,
        len,
        INST_WRITE,
        ADDR_GOAL_POSITION,
        pos_l,
        pos_h,
        !sum,
    ]
}

async fn drive_to(tx: &mut UartTx<'static, Async>, pos: u16) {
    let pkt = goal_position_packet(SERVO_ID, pos);
    if tx.write_async(&pkt).await.is_ok() {
        let _ = tx.flush_async().await;
    }
}

#[embassy_executor::task]
pub async fn run(mut tx: UartTx<'static, Async>) {
    let mut manual_was = false;
    let mut last_pos = 0u16;
    loop {
        // Start only when the beer byte has visually travelled the strip and
        // reached the servo. Travel time follows NUM_LEDS x the configurable
        // LED speed, so the sync stays correct for any strip length / speed.
        // Consume the arrival event, but only run the pour sequence outside
        // manual mode (in manual the byte is just a visual flourish per tick).
        let arrived = control::take_beer_arrived();
        if arrived && !control::manual_on() {
            control::set_beer_pouring(true); // keep BEER indicator lit during pour
            for &pos in SEQUENCE.iter() {
                drive_to(&mut tx, pos).await;
                Timer::after(Duration::from_millis(STEP_MS)).await;
            }
            control::set_beer_pouring(false);
        }
        // BEER MANUAL: drive only when the encoder actually changes the target.
        // On entry, sync the last value without moving (don't jump on open).
        if control::manual_on() {
            let pos = control::servo_pos() as u16;
            if !manual_was {
                last_pos = pos;
            } else if pos != last_pos {
                drive_to(&mut tx, pos).await;
                last_pos = pos;
            }
            manual_was = true;
        } else {
            manual_was = false;
        }
        Timer::after(Duration::from_millis(20)).await;
    }
}
