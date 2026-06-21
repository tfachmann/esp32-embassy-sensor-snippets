//! Real-time 3D axis gizmo (X, Y, Z; Z up) visualizing the board's tilt.
//! Board-frame unit axes are rotated by the IMU pitch/roll, then projected with
//! a simple isometric projection and drawn as labelled lines.

use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Alignment, Text};
use libm::{cosf, fabsf, sinf, sqrtf};

const CX: f32 = 64.0; // screen origin
const CY: f32 = 33.0;
const SCALE: f32 = 18.0; // axis length in pixels (kept small so it always fits)
const C30: f32 = 0.866_025_4; // cos 30
const S30: f32 = 0.5; // sin 30

const BAR_W: i32 = 5; // dynamic-accel (motion) magnitude bar on the left column
const BAR_MAX_G: f32 = 1.0; // full scale for |accel| deviation from 1g

fn rotate(v: [f32; 3], pitch: f32, roll: f32) -> [f32; 3] {
    let (sp, cp) = (sinf(pitch), cosf(pitch));
    let (sr, cr) = (sinf(roll), cosf(roll));
    let [x, y, z] = v;
    // Rx(roll)
    let x1 = x;
    let y1 = y * cr - z * sr;
    let z1 = y * sr + z * cr;
    // Ry(pitch)
    let x2 = x1 * cp + z1 * sp;
    let y2 = y1;
    let z2 = -x1 * sp + z1 * cp;
    [x2, y2, z2]
}

fn project(v: [f32; 3]) -> Point {
    let [x, y, z] = v;
    let sx = CX + SCALE * (x - y) * C30;
    let sy = CY + SCALE * ((x + y) * S30 - z); // z up -> smaller sy
    Point::new(sx as i32, sy as i32)
}

pub fn render<D>(
    display: &mut D,
    pitch_deg: i32,
    roll_deg: i32,
    ax: f32,
    ay: f32,
    az: f32,
    text: MonoTextStyle<'_, BinaryColor>,
) where
    D: DrawTarget<Color = BinaryColor>,
{
    let pitch = (pitch_deg as f32).to_radians();
    let roll = (roll_deg as f32).to_radians();
    let origin = project([0.0, 0.0, 0.0]);
    let stroke = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

    // Dynamic-acceleration bar (far-left column), bottom-aligned: deviation of
    // the total accel magnitude from 1g, so it is ~0 when held still in any
    // orientation and rises with motion/shake.
    let mag = fabsf(sqrtf(ax * ax + ay * ay + az * az) - 1.0);
    let _ = Rectangle::new(Point::new(0, 1), Size::new(BAR_W as u32, 62))
        .into_styled(stroke)
        .draw(display);
    let fill = ((mag / BAR_MAX_G).clamp(0.0, 1.0) * 60.0) as i32;
    if fill > 0 {
        let _ = Rectangle::new(Point::new(1, 62 - fill), Size::new((BAR_W - 2) as u32, fill as u32))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display);
    }

    for (v, label) in [
        ([1.0, 0.0, 0.0], "X"),
        ([0.0, 1.0, 0.0], "Y"),
        ([0.0, 0.0, 1.0], "Z"),
    ] {
        let tip = project(rotate(v, pitch, roll));
        let _ = Line::new(origin, tip).into_styled(stroke).draw(display);
        // label just past the tip, clamped into a safe inner rect so the glyph
        // (centered, ~6x10) never clips at the panel edges.
        let lx = (tip.x + (tip.x - origin.x).signum() * 4).clamp(6, 121);
        let ly = (tip.y + (tip.y - origin.y).signum() * 4).clamp(9, 62);
        let _ = Text::with_alignment(label, Point::new(lx, ly), text, Alignment::Center)
            .draw(display);
    }
}
