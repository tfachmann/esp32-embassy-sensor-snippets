//! Nyancat easter-egg animation: 12 frames, 128x64 1-bit, played full-screen.
//! Frames are 1024-byte ImageRawLE blobs (from esp32-nyancat-embassy).

use embedded_graphics::image::ImageRawLE;
use embedded_graphics::pixelcolor::BinaryColor;

const NYAN_FRAME_MS: u32 = 90; // per-frame hold (wall-clock indexed)

macro_rules! load_images {
    ($($i:literal),*) => {
        [ $( ImageRawLE::new(include_bytes!(concat!("assets/nyancat/", $i, ".raw")), 128), )* ]
    };
}

static IMAGES: [ImageRawLE<'_, BinaryColor>; 12] =
    load_images!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12);

/// The frame to show at `now_ms` (wall-clock indexed -> constant speed).
pub fn frame(now_ms: u32) -> &'static ImageRawLE<'static, BinaryColor> {
    &IMAGES[((now_ms / NYAN_FRAME_MS) % IMAGES.len() as u32) as usize]
}
