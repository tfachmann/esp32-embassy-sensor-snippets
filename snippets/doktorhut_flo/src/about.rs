//! About screen: 64x64 1-bit logo on the left, message on the right.

use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};

// 64x64, 1bpp, MSB-first (matches ImageRaw<BinaryColor>). See share/lnm_64.png.
static LOGO: &[u8] = include_bytes!("lnm_64.raw");

pub fn render<D>(
    display: &mut D,
    text_style: MonoTextStyle<'_, BinaryColor>,
    small_style: MonoTextStyle<'_, BinaryColor>,
) where
    D: DrawTarget<Color = BinaryColor>,
{
    let img = ImageRaw::<BinaryColor>::new(LOGO, 64);
    let _ = Image::new(&img, Point::new(0, 0)).draw(display);
    let _ = Text::with_baseline(
        "You\nwill be\nmissed",
        Point::new(70, 12),
        text_style,
        Baseline::Top,
    )
    .draw(display);
    let _ = Text::with_baseline(
        "DLR\n2010 - 2026",
        Point::new(70, 48),
        small_style,
        Baseline::Top,
    )
    .draw(display);
}
