//! FLIP fluid animation (fluid_sim crate, MIT, Nicholas-L-Johnson/flip-card)
//! tilted by the IMU. The `Scene` is tens of KB of fixed arrays, so it lives in
//! a static -- never the embassy task arena.

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU32, Ordering};

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use fluid_sim::FluidSimulation::Scene;

const PARTICLES: i32 = 350; // tune for speed vs. fill (vendored grid seeds 350)
const GRID_W: usize = 32; // get_output() is 32x14 (vendored 34x16 grid - 2)
const GRID_H: usize = 14;
const GRAVITY_SCALE: f32 = 20.0; // accel (g) -> sim gravity (>9.81 = livelier)
const FLIP_RATIO: f32 = 0.90; // FLIP/PIC blend = viscosity: lower = more viscous
const MAX_SUBSTEPS: u32 = 6; // clamp so a slow frame can't death-spiral

// The Scene is ~30-50KB of fixed arrays. We build it IN PLACE in this static
// (zero the bytes, then set the non-zero fields) so there is no by-value stack
// construction -- that ~50KB stack peak is what crashed FLUIDS at high res.
static mut SCENE: MaybeUninit<Scene> = MaybeUninit::uninit();
static LAST_MS: AtomicU32 = AtomicU32::new(0);

pub fn init() -> &'static mut Scene {
    // SAFETY: called once at startup; the returned &'static mut is used only by
    // the single display task. All-zero is a valid Scene (cellType = AIR_CELL = 0,
    // f32/i32 = 0, bools = false, no Drop types); setup_in_place then fills the
    // non-zero fields (scalars, solid mask, particle seed).
    unsafe {
        let p = core::ptr::addr_of_mut!(SCENE) as *mut Scene;
        core::ptr::write_bytes(p as *mut u8, 0, core::mem::size_of::<Scene>());
        let scene = &mut *p;
        scene.setup_in_place(PARTICLES);
        scene.set_flip_ratio(FLIP_RATIO);
        scene
    }
}

/// Step the sim and render the grid centered inside the region `(ox, oy, w, h)`,
/// auto-sizing the cell to fit.
pub fn step_and_render<D>(scene: &mut Scene, display: &mut D, ax: f32, ay: f32, ox: i32, oy: i32, w: i32, h: i32)
where
    D: DrawTarget<Color = BinaryColor>,
{
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
    let cell = (w / GRID_W as i32).min(h / GRID_H as i32).max(1);
    let x_off = ox + (w - GRID_W as i32 * cell) / 2;
    let y_off = oy + (h - GRID_H as i32 * cell) / 2;
    let style = PrimitiveStyle::with_fill(BinaryColor::On);
    for (y, row) in grid.iter().enumerate() {
        for (x, &on) in row.iter().enumerate() {
            if on {
                let px = x_off + x as i32 * cell;
                let py = y_off + y as i32 * cell;
                let _ = Rectangle::new(Point::new(px, py), Size::new(cell as u32, cell as u32))
                    .into_styled(style)
                    .draw(display);
            }
        }
    }
}
