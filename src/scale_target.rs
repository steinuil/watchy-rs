use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Dimensions, Size},
    prelude::Pixel,
    primitives::Rectangle,
};

pub struct ScaleTarget<'a, D> {
    target: &'a mut D,
    scale: u32,
}

impl<'a, D: DrawTarget> ScaleTarget<'a, D> {
    pub fn new(target: &'a mut D, scale: u32) -> Self {
        Self { target, scale }
    }

    fn scale_rect(&self, area: &Rectangle) -> Rectangle {
        Rectangle::new(area.top_left * self.scale as i32, area.size * self.scale)
    }
}

impl<D: DrawTarget> Dimensions for ScaleTarget<'_, D> {
    fn bounding_box(&self) -> Rectangle {
        let bb = self.target.bounding_box();
        Rectangle::new(bb.top_left / self.scale as i32, bb.size / self.scale)
    }
}

impl<D: DrawTarget> DrawTarget for ScaleTarget<'_, D> {
    type Color = D::Color;
    type Error = D::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let block = Size::new_equal(self.scale);

        for Pixel(point, color) in pixels {
            let area = Rectangle::new(point * self.scale as i32, block);
            self.target.fill_solid(&area, color)?;
        }

        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let area = self.scale_rect(area);
        self.target.fill_solid(&area, color)
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.target.clear(color)
    }
}
