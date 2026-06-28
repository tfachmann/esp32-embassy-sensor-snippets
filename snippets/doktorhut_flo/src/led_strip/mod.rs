//! WS2812B LED strip: visualize bytes travelling down a wire.

mod driver;
pub mod effects;

use embassy_time::{Duration, Instant, Timer};
use libm::{fabsf, sqrtf};

use crate::control;
pub use driver::{new_rmt, Ws2812};
use effects::{BeerByte, Effect, Stream};

/// Compile-time strip length. The pulse buffer (`NUM_LEDS * 24 + 1` u32) lives
/// in the task arena; the default 20480 holds up to ~100 LEDs.
pub const NUM_LEDS: usize = 60;

/// RGB pixel; driver reorders to WS2812B GRB.
pub type Rgb = [u8; 3];
pub type Framebuffer = [Rgb; NUM_LEDS];

const FRAME_MS: u64 = 8;

const BEER_BYTE: u8 = 0b1011_0010;
// Beer byte travels at this fraction of the configured LED speed (slower than
// the stream, but still scales with it -> servo triggers earlier when faster).
const BEER_SPEED_DIV: u32 = 4;
// Beer byte velocity (Q8.8 LEDs/frame) in BEER MANUAL: fast, but not instant.
const BEER_MANUAL_VEL_Q8: i32 = 4 * 256;

/// Each physical strip is dedicated to one process.
#[derive(Clone, Copy, PartialEq)]
pub enum StripRole {
    Beer,
    Imu,
    Music,
}

const MUSIC_COLOR: Rgb = [0, 255, 90]; // green pulse while music plays
const MUSIC_PERIOD_MS: u32 = 2000; // breathing cycle

#[embassy_executor::task(pool_size = 3)]
pub async fn run(mut strip: Ws2812, role: StripRole) {
    let mut fb: Framebuffer = [[0, 0, 0]; NUM_LEDS];

    let mut stream = Stream::new();
    let mut beer = BeerByte::new(BEER_BYTE);
    let mut beer_was_on = false;

    loop {
        let now = Instant::now().as_millis() as u32;
        fb.fill([0, 0, 0]);

        // PARTY easter egg overrides every strip with a rainbow.
        if control::party_on() {
            party_render(&mut fb, role, now);
            strip.write(&fb);
            Timer::after(Duration::from_millis(FRAME_MS)).await;
            continue;
        }

        match role {
            // IMU strip: the byte stream while the IMU is on, brightness ramping
            // up after it starts.
            StripRole::Imu => {
                if control::imu_on() {
                    stream.set_velocity_q8(control::velocity_q8());
                    stream.render(&mut fb);
                    let ramp = control::imu_ramp_q8(Instant::now().as_millis() as u32);
                    if ramp < 256 {
                        for px in fb.iter_mut() {
                            for c in px.iter_mut() {
                                *c = (*c as u32 * ramp / 256) as u8;
                            }
                        }
                    }
                }
            }

            // BEER strip: a single distinct-color byte travelling the strip; it
            // also signals the servo when it reaches the end.
            StripRole::Beer => {
                let beer_on = control::beer_on();
                if beer_on && !beer_was_on {
                    beer.reset(); // rising edge -> fire a new shot
                }
                beer_was_on = beer_on;
                if beer_on {
                    let beer_vel = if control::manual_on() {
                        BEER_MANUAL_VEL_Q8
                    } else {
                        (control::velocity_q8() / BEER_SPEED_DIV).max(1) as i32
                    };
                    beer.overlay(&mut fb, beer_vel);
                    if beer.finished() {
                        control::signal_beer_arrived();
                        control::clear_beer();
                    }
                }
            }

            // MUSIC strip: a gentle breathing pulse while music plays. Phase from
            // wall-clock time (not a per-frame counter), so it stays smooth even if
            // core1's frame cadence stutters. (Placeholder until the multi-track
            // music refactor.)
            StripRole::Music => {
                if control::music_on() {
                    let t = Instant::now().as_millis() as u32;
                    let phase = (t % MUSIC_PERIOD_MS) * 256 / MUSIC_PERIOD_MS; // 0..255
                    let tri = if phase < 128 { phase * 2 } else { (255 - phase) * 2 };
                    let level = 40 + tri * 215 / 255; // keep a dim floor
                    let color = [
                        (MUSIC_COLOR[0] as u32 * level / 255) as u8,
                        (MUSIC_COLOR[1] as u32 * level / 255) as u8,
                        (MUSIC_COLOR[2] as u32 * level / 255) as u8,
                    ];
                    fb.fill(color);
                }
            }
        }

        strip.write(&fb);
        Timer::after(Duration::from_millis(FRAME_MS)).await;
    }
}

/// PARTY: a fast-scrolling rainbow with a tilt-driven overlay. The BEER strip
/// shows a bright white "gravity ball" (framed by an off-LED on each side for
/// pop) rolling to the downhill end; the other strips show a "liquid level"
/// that fills up to the tilt angle. Shake (accel) blends everything white.
fn party_render(fb: &mut Framebuffer, role: StripRole, now: u32) {
    let role_off: u32 = match role {
        StripRole::Beer => 0,
        StripRole::Imu => 85,
        StripRole::Music => 170,
    };
    let scroll = now / 8; // fast rainbow scroll
    // Tilt (roll -90..90) -> a position along the 60-LED strip.
    let roll = control::roll().clamp(-90, 90);
    let pos = (roll + 90) * (NUM_LEDS as i32 - 1) / 180; // 0..NUM_LEDS-1
    // Shake -> white blend.
    let (ax, ay, az) = (control::accel_x(), control::accel_y(), control::accel_z());
    let mag = fabsf(sqrtf(ax * ax + ay * ay + az * az) - 1.0);
    let strobe = (mag.clamp(0.0, 1.0) * 255.0) as u32; // 0..255
    let shift = control::brightness_shift();
    let ball = role == StripRole::Beer;
    const BALL_R: i32 = 4; // solid white blob half-width
    const BALL_GAP: i32 = 2; // off-LEDs framing each side

    for (i, px) in fb.iter_mut().enumerate() {
        let hue = (scroll + role_off + (i as u32 * 256 / NUM_LEDS as u32)) as u8;
        let (r0, g0, b0) = hsv_to_rgb(hue, 255, 220);
        let idx = i as i32;

        // White amount (0..255) and brightness (0..255) from the tilt overlay.
        let (white, bright) = if ball {
            let d = (idx - pos).abs();
            if d <= BALL_R {
                (255, 255) // solid bright-white ball (high contrast)
            } else if d <= BALL_R + BALL_GAP {
                (0, 0) // dark frame on each side so the ball stands out
            } else {
                (0, 14) // very dim rainbow background
            }
        } else if idx <= pos {
            (0, 255) // liquid: bright up to the level
        } else {
            (0, 45) // liquid: dim above
        };
        let bright = bright.max(strobe); // a hard shake lights the whole strip
        let w = white.max(strobe);

        let mix = |c: u8| -> u8 {
            let lit = c as u32 * bright / 255; // apply brightness
            let lifted = lit + (255 - lit) * w / 255; // blend toward white
            (lifted as u8) >> shift
        };
        *px = [mix(r0), mix(g0), mix(b0)];
    }
}

/// 8-bit HSV -> RGB (hue 0..255).
fn hsv_to_rgb(h: u8, s: u8, v: u8) -> (u8, u8, u8) {
    if s == 0 {
        return (v, v, v);
    }
    let region = (h / 43) as u32;
    let rem = (h % 43) as u32 * 6; // 0..252
    let p = (v as u32 * (255 - s as u32) / 255) as u8;
    let q = (v as u32 * (255 - s as u32 * rem / 255) / 255) as u8;
    let t = (v as u32 * (255 - s as u32 * (255 - rem) / 255) / 255) as u8;
    match region {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}
