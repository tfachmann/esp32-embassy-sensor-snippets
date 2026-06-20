mod packet;

pub use packet::Packet;

use crate::led_strip::Framebuffer;

pub trait Effect {
    fn render(&mut self, fb: &mut Framebuffer);
}
