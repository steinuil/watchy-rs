use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{DrawTarget, Point, Size},
    primitives::{Primitive, PrimitiveStyle, Rectangle, Styled},
    transform::Transform,
    Drawable,
};

fn rect<I: Into<i32>, I2: Into<u32>>(
    x: I,
    y: I,
    w: I2,
    h: I2,
) -> Styled<Rectangle, PrimitiveStyle<BinaryColor>> {
    Rectangle::new(
        Point::new(x.into(), y.into()),
        Size::new(w.into(), h.into()),
    )
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
}

pub fn letter<E, T: DrawTarget<Color = BinaryColor, Error = E>>(
    c: char,
    (w, h): (u16, u16),
    (ws, hs): (u16, u16),
    translate: Point,
    target: &mut T,
) -> Result<(), E> {
    match c {
        '0' => {
            rect(0, 0, w * 3 + ws * 2, h)
                .translate(translate)
                .draw(target)?;
            rect(0, h, w, h * 2 + hs * 2)
                .translate(translate)
                .draw(target)?;
            rect(w * 2 + ws * 2, h, w, h * 2 + hs * 2)
                .translate(translate)
                .draw(target)?;
        }
        _ => {}
    }

    Ok(())
}
