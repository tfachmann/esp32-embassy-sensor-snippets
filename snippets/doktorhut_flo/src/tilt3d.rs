//! Real-time 3D axis gizmo (X, Y, Z; Z up) visualizing the board's tilt.
//! Board-frame unit axes are rotated by the IMU pitch/roll, then projected with
//! a simple isometric projection and drawn as labelled lines.

use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Alignment, Text};
use libm::{cosf, fabsf, sinf, sqrtf};

const C30: f32 = 0.866_025_4; // cos 30
const S30: f32 = 0.5; // sin 30

const BAR_W: i32 = 8; // dynamic-accel (motion) magnitude bar on the left
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

/// Render the gizmo + accel bar inside the rectangle `(ox, oy, w, h)`.
pub fn render<D>(
    display: &mut D,
    pitch_deg: i32,
    roll_deg: i32,
    ax: f32,
    ay: f32,
    az: f32,
    text: MonoTextStyle<'_, BinaryColor>,
    ox: i32,
    oy: i32,
    w: i32,
    h: i32,
) where
    D: DrawTarget<Color = BinaryColor>,
{
    let stroke = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

    // Dynamic-acceleration bar at the window's left edge.
    let mag = fabsf(sqrtf(ax * ax + ay * ay + az * az) - 1.0);
    let bar_h = h - 4;
    let _ = Rectangle::new(Point::new(ox + 2, oy + 2), Size::new(BAR_W as u32, bar_h as u32))
        .into_styled(stroke)
        .draw(display);
    let fill = ((mag / BAR_MAX_G).clamp(0.0, 1.0) * (bar_h - 2) as f32) as i32;
    if fill > 0 {
        let _ = Rectangle::new(
            Point::new(ox + 3, oy + 2 + bar_h - 1 - fill),
            Size::new((BAR_W - 2) as u32, fill as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display);
    }

    // Gizmo centered in the window (offset right of the bar), scaled to fit.
    let cx = (ox + BAR_W + 4 + (w + ox - (ox + BAR_W + 4)) / 2) as f32;
    let cy = (oy + h / 2) as f32;
    let scale = (h.min(w - BAR_W - 6) as f32) * 0.32;
    let project = |v: [f32; 3]| -> Point {
        let [x, y, z] = v;
        let sx = cx + scale * (x - y) * C30;
        let sy = cy + scale * ((x + y) * S30 - z);
        Point::new(sx as i32, sy as i32)
    };

    let pitch = (pitch_deg as f32).to_radians();
    let roll = (roll_deg as f32).to_radians();
    let origin = project([0.0, 0.0, 0.0]);
    let (lo_x, hi_x) = (ox + BAR_W + 6, ox + w - 4);
    let (lo_y, hi_y) = (oy + 6, oy + h - 3);

    for (v, label) in [
        ([1.0, 0.0, 0.0], "X"),
        ([0.0, 1.0, 0.0], "Y"),
        ([0.0, 0.0, 1.0], "Z"),
    ] {
        let tip = project(rotate(v, pitch, roll));
        let _ = Line::new(origin, tip).into_styled(stroke).draw(display);
        let lx = (tip.x + (tip.x - origin.x).signum() * 4).clamp(lo_x, hi_x);
        let ly = (tip.y + (tip.y - origin.y).signum() * 4).clamp(lo_y, hi_y);
        let _ =
            Text::with_alignment(label, Point::new(lx, ly), text, Alignment::Center).draw(display);
    }
}
