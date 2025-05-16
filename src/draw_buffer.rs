use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{DrawTarget, OriginDimensions, Size},
    Pixel,
};

const WIDTH: usize = 200;

pub struct DrawBuffer([u8; WIDTH * WIDTH / 8]);

impl DrawBuffer {
    pub fn empty() -> Self {
        DrawBuffer([0xFF; WIDTH * WIDTH / 8])
    }

    pub fn buffer(&self) -> &[u8] {
        &self.0
    }
}

impl OriginDimensions for DrawBuffer {
    fn size(&self) -> embedded_graphics::prelude::Size {
        Size {
            width: WIDTH as u32,
            height: WIDTH as u32,
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
                let index = x as usize + y as usize * WIDTH;
                self.0[index / 8] &= !(1 << (7 - (index % 8)));
                if color.is_off() {
                    self.0[index / 8] |= 1 << (7 - (index % 8));
                }
            }
        }

        Ok(())
    }
}
