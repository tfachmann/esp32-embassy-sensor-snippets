//! FLIP fluid animation (fluid_sim crate, MIT, Nicholas-L-Johnson/flip-card)
//! tilted by the IMU. The `Scene` is tens of KB of fixed arrays, so it lives in
//! a static -- never the embassy task arena.

use core::sync::atomic::{AtomicU32, Ordering};

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use fluid_sim::FluidSimulation::Scene;
use static_cell::StaticCell;

const PARTICLES: i32 = 200; // tune for speed vs. fill (vendored grid seeds 200)
const GRID_W: usize = 24; // get_output() is 24x12 (vendored 26x14 grid - 2)
const GRID_H: usize = 12;
const CELL: i32 = 5; // pixels per cell -> 120x60, near-full 128x64
const GRAVITY_SCALE: f32 = 25.0; // accel (g) -> sim gravity (>9.81 = livelier)
const MAX_SUBSTEPS: u32 = 6; // clamp so a slow frame can't death-spiral

static SCENE: StaticCell<Scene> = StaticCell::new();

pub fn init() -> &'static mut Scene {
    SCENE.init(Scene::setupScene(PARTICLES))
}

static DBG: AtomicU32 = AtomicU32::new(0);
static LAST_MS: AtomicU32 = AtomicU32::new(0);

/// Print the occupancy grid + accel to serial (~every second) to debug the sim.
fn debug_dump(grid: &[[bool; GRID_W]; GRID_H], ax: f32, ay: f32) {
    if DBG.fetch_add(1, Ordering::Relaxed) % 25 != 0 {
        return;
    }
    esp_println::println!("--- fluid ax={ax:.2} ay={ay:.2} ---");
    for row in grid.iter() {
        let mut line = [b'.'; GRID_W];
        for (x, &on) in row.iter().enumerate() {
            if on {
                line[x] = b'#';
            }
        }
        esp_println::println!("{}", core::str::from_utf8(&line).unwrap_or(""));
    }
}

pub fn step_and_render<D>(scene: &mut Scene, display: &mut D, ax: f32, ay: f32)
where
    D: DrawTarget<Color = BinaryColor>,
{
    // Map IMU acceleration to the sim's gravity vector. Sign/axis tuned so that
    // tilting the board makes the fluid fall the expected way.
    scene.set_gravity([ax * GRAVITY_SCALE, -ay * GRAVITY_SCALE]);

    // Advance the sim by the real wall-clock time elapsed since the last frame
    // (each simulate() = dt = 1/60 s), so motion is real-time regardless of the
    // flush-limited frame rate.
    let now = embassy_time::Instant::now().as_millis() as u32;
    let last = LAST_MS.swap(now, Ordering::Relaxed);
    let dt_ms = now.wrapping_sub(last).min(200); // cap first call / long pauses
    let steps = ((dt_ms * 60 + 500) / 1000).clamp(1, MAX_SUBSTEPS);
    for _ in 0..steps {
        scene.simulate();
    }

    let grid = scene.get_output(); // [y][x] occupancy
    debug_dump(&grid, ax, ay);
    let x_off = (128 - GRID_W as i32 * CELL) / 2;
    let y_off = (64 - GRID_H as i32 * CELL) / 2;
    let style = PrimitiveStyle::with_fill(BinaryColor::On);
    for (y, row) in grid.iter().enumerate() {
        for (x, &on) in row.iter().enumerate() {
            if on {
                let px = x_off + x as i32 * CELL;
                let py = y_off + y as i32 * CELL;
                let _ = Rectangle::new(Point::new(px, py), Size::new(CELL as u32, CELL as u32))
                    .into_styled(style)
                    .draw(display);
            }
        }
    }
}
