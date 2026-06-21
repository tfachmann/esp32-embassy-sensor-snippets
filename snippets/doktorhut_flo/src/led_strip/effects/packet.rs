//! One byte propagating down the wire, MSB first, looping forever.
//! Motion uses a Q8.8 fixed-point head position advanced by a per-frame
//! velocity; size = `bit_width` (LEDs/bit).

use super::Effect;
use crate::led_strip::{Framebuffer, Rgb, NUM_LEDS};

const BITS: i32 = 8;
const Q8: i32 = 256;

const OFF: Rgb = [0, 0, 0];
const BIT_ONE: Rgb = [0, 200, 255];
const BIT_ZERO: Rgb = [16, 0, 28];

pub struct Packet {
    byte: u8,
    pos_q8: i32,
    vel_q8: i32,
    bit_width: i32,
}

impl Packet {
    pub fn new(byte: u8) -> Self {
        let bit_width = 1;
        Self {
            byte,
            pos_q8: -BITS * bit_width * Q8,
            vel_q8: Q8 / 4,
            bit_width,
        }
    }

    pub fn with_bit_width(mut self, width: u32) -> Self {
        self.bit_width = width.max(1) as i32;
        self.pos_q8 = -self.span() * Q8;
        self
    }

    fn span(&self) -> i32 {
        BITS * self.bit_width
    }
}

impl Effect for Packet {
    fn set_velocity_q8(&mut self, vel_q8: u32) {
        self.vel_q8 = (vel_q8 as i32).max(1);
    }

    fn render(&mut self, fb: &mut Framebuffer) {
        fb.fill(OFF);

        let head = self.pos_q8 >> 8; // arithmetic shift floors toward -inf
        for k in 0..BITS {
            let bit = (self.byte >> (7 - k)) & 1;
            let color = if bit == 1 { BIT_ONE } else { BIT_ZERO };
            let bit_start = head + k * self.bit_width;
            for w in 0..self.bit_width {
                let idx = bit_start + w;
                if idx < 0 || idx >= NUM_LEDS as i32 {
                    continue;
                }
                fb[idx as usize] = color;
            }
        }

        self.pos_q8 += self.vel_q8;
        if self.pos_q8 >> 8 >= NUM_LEDS as i32 {
            self.pos_q8 = -self.span() * Q8;
        }
    }
}
