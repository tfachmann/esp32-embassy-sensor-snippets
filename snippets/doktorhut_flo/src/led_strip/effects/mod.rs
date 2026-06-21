mod beer;
mod packet;
mod stream;

pub use beer::BeerByte;
pub use packet::Packet;
pub use stream::Stream;

use crate::led_strip::Framebuffer;

pub trait Effect {
    fn render(&mut self, fb: &mut Framebuffer);
    fn set_velocity_q8(&mut self, vel_q8: u32);
}
