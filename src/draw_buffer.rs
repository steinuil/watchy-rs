use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{DrawTarget, OriginDimensions, Size},
    Pixel,
};

use crate::display::Display;

pub struct DrawBuffer(pub [u8; Display::FRAME_LEN]);

impl DrawBuffer {
    pub fn empty() -> Self {
        DrawBuffer([0xFF; Display::FRAME_LEN])
    }

    pub fn as_array(&self) -> &[u8; Display::FRAME_LEN] {
        &self.0
    }

    pub fn clear(&mut self) {
        self.0.fill(0xFF);
    }
}

impl OriginDimensions for DrawBuffer {
    fn size(&self) -> embedded_graphics::prelude::Size {
        Size {
            width: Display::WIDTH as u32,
            height: Display::HEIGHT as u32,
        }
    }
}

impl DrawTarget for DrawBuffer {
    type Color = BinaryColor;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(pos, color) in pixels.into_iter() {
            if let (x @ 0..=199, y @ 0..=199) = pos.into() {
                let index = x as usize + y as usize * Display::WIDTH as usize;
                self.0[index / 8] &= !(1 << (7 - (index % 8)));
                if color.is_off() {
                    self.0[index / 8] |= 1 << (7 - (index % 8));
                }
            }
        }

        Ok(())
    }
}
