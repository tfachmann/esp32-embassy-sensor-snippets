//! The "BEER" shot: a single byte fired once down the wire in a distinct color,
//! overlaid on whatever else is on the strip. Travels slowly, then reports
//! `finished()`.

use crate::led_strip::{Framebuffer, Rgb, NUM_LEDS};

const BITS: i32 = 8;
const Q8: i32 = 256;
const VEL_Q8: i32 = Q8 / 16; // slow: ~0.06 LED/frame

// Distinct from the cyan/violet stream.
const BEER_ONE: Rgb = [255, 80, 0]; // orange
const BEER_ZERO: Rgb = [40, 12, 0];

pub struct BeerByte {
    byte: u8,
    pos_q8: i32,
    done: bool,
}

impl BeerByte {
    pub fn new(byte: u8) -> Self {
        Self {
            byte,
            pos_q8: -BITS * Q8,
            done: true,
        }
    }

    /// (Re)start the shot from off-screen left.
    pub fn reset(&mut self) {
        self.pos_q8 = -BITS * Q8;
        self.done = false;
    }

    pub fn finished(&self) -> bool {
        self.done
    }

    /// Overlay the traveling byte onto `fb` (does not clear it), advancing once.
    pub fn overlay(&mut self, fb: &mut Framebuffer) {
        if self.done {
            return;
        }
        let head = self.pos_q8 >> 8;
        for k in 0..BITS {
            let bit = (self.byte >> (7 - k)) & 1;
            let idx = head + k;
            if idx >= 0 && idx < NUM_LEDS as i32 {
                fb[idx as usize] = if bit == 1 { BEER_ONE } else { BEER_ZERO };
            }
        }
        self.pos_q8 += VEL_Q8;
        if self.pos_q8 >> 8 >= NUM_LEDS as i32 {
            self.done = true;
        }
    }
}
