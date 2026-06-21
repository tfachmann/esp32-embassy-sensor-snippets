//! Continuous byte stream scrolling down the wire, MSB first, with random OFF
//! gaps between bytes. Cells scroll in from the left at the shared velocity.

use super::Effect;
use crate::led_strip::{Framebuffer, Rgb, NUM_LEDS};

const Q8: i32 = 256;

const OFF: Rgb = [0, 0, 0];
const BIT_ONE: Rgb = [0, 200, 255];
const BIT_ZERO: Rgb = [16, 0, 28];

const GAP_MIN: u32 = 2;
const GAP_MAX: u32 = 8;

struct Rng(u32);

impl Rng {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + self.next_u32() % (hi - lo + 1)
    }
}

#[derive(Clone, Copy)]
enum Src {
    Bits { byte: u8, left: u8 },
    Gap { left: u32 },
}

pub struct Stream {
    cells: [Rgb; NUM_LEDS],
    pos_q8: i32,
    vel_q8: i32,
    rng: Rng,
    src: Src,
}

impl Stream {
    pub fn new() -> Self {
        Self {
            cells: [OFF; NUM_LEDS],
            pos_q8: 0,
            vel_q8: Q8 / 4,
            rng: Rng(0x1234_5678),
            src: Src::Gap { left: GAP_MIN },
        }
    }

    fn next_pixel(&mut self) -> Rgb {
        match self.src {
            Src::Bits { byte, left } => {
                let bit = (byte >> (left - 1)) & 1;
                let color = if bit == 1 { BIT_ONE } else { BIT_ZERO };
                self.src = if left > 1 {
                    Src::Bits {
                        byte,
                        left: left - 1,
                    }
                } else {
                    Src::Gap {
                        left: self.rng.range(GAP_MIN, GAP_MAX),
                    }
                };
                color
            }
            Src::Gap { left } => {
                self.src = if left > 1 {
                    Src::Gap { left: left - 1 }
                } else {
                    Src::Bits {
                        byte: self.rng.next_u32() as u8,
                        left: 8,
                    }
                };
                OFF
            }
        }
    }

    fn step(&mut self) {
        for i in (1..NUM_LEDS).rev() {
            self.cells[i] = self.cells[i - 1];
        }
        self.cells[0] = self.next_pixel();
    }
}

impl Effect for Stream {
    fn set_velocity_q8(&mut self, vel_q8: u32) {
        self.vel_q8 = (vel_q8 as i32).max(1);
    }

    fn render(&mut self, fb: &mut Framebuffer) {
        self.pos_q8 += self.vel_q8;
        while self.pos_q8 >= Q8 {
            self.pos_q8 -= Q8;
            self.step();
        }
        fb.copy_from_slice(&self.cells);
    }
}
