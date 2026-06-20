//! One byte propagating down the wire, MSB first, looping forever.
//! speed = `step_frames` (frames per LED step), size = `bit_width` (LEDs/bit).

use super::Effect;
use crate::led_strip::{Framebuffer, Rgb, NUM_LEDS};

const BITS: i32 = 8;

const OFF: Rgb = [0, 0, 0];
const BIT_ONE: Rgb = [0, 200, 255];
const BIT_ZERO: Rgb = [16, 0, 28];

pub struct Packet {
    byte: u8,
    head: i32,
    step_frames: u32,
    bit_width: i32,
    counter: u32,
}

impl Packet {
    pub fn new(byte: u8) -> Self {
        let bit_width = 1;
        Self {
            byte,
            head: -BITS * bit_width,
            step_frames: 6,
            bit_width,
            counter: 0,
        }
    }

    pub fn with_step_frames(mut self, frames: u32) -> Self {
        self.step_frames = frames.max(1);
        self
    }

    pub fn with_bit_width(mut self, width: u32) -> Self {
        self.bit_width = width.max(1) as i32;
        self.head = -BITS * self.bit_width;
        self
    }

    fn span(&self) -> i32 {
        BITS * self.bit_width
    }
}

impl Effect for Packet {
    fn render(&mut self, fb: &mut Framebuffer) {
        fb.fill(OFF);

        for k in 0..BITS {
            let bit = (self.byte >> (7 - k)) & 1;
            let color = if bit == 1 { BIT_ONE } else { BIT_ZERO };
            let bit_start = self.head + k * self.bit_width;
            for w in 0..self.bit_width {
                let idx = bit_start + w;
                if idx < 0 || idx >= NUM_LEDS as i32 {
                    continue;
                }
                fb[idx as usize] = color;
            }
        }

        self.counter += 1;
        if self.counter >= self.step_frames {
            self.counter = 0;
            self.head += 1;
            if self.head >= NUM_LEDS as i32 {
                self.head = -self.span();
            }
        }
    }
}
