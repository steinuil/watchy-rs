#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use arrayvec::ArrayString;
use core::fmt::Write as _;
use defmt::println;
use embassy_executor::Spawner;
use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::BinaryColor,
    prelude::{Point, Primitive, Size, Transform},
    primitives::{Circle, PrimitiveStyle, Rectangle, Triangle},
    text::Text,
    Drawable as _,
};
use esp_backtrace as _;
use esp_hal_embassy::main;
use esp_println as _;
use unwrap_infallible::UnwrapInfallible as _;
use watchy::{WakeupCause, Watchy};

mod battery;
mod buttons;
mod draw_buffer;
mod font;
mod vibration_motor;
pub mod watchy;

#[main]
async fn main(_spawner: Spawner) {
    let mut watchy = match Watchy::init() {
        Ok(watchy) => watchy,
        Err(error) => {
            println!("{:?}", error);
            return;
        }
    };

    println!("watchy initialized");

    if let WakeupCause::Reset = watchy.get_wakeup_cause() {
        watchy.sensor.initialize().await.unwrap();
        println!("initialized sensor")
    }

    let time = watchy.external_rtc.read_time().await.unwrap();

    let voltage = watchy.battery.voltage().await;
    let percentage = ((voltage - 2.75) / (3.7 - 2.75)) * 100.0;
    println!("battery voltage: {}", voltage);
    println!("battery percentage: {}", percentage);

    let temperature = watchy
        .sensor
        .temperature_celsius()
        .await
        .unwrap()
        .unwrap_or_default();
    println!("temperature: {}", temperature);

    let (x, y, z) = watchy.sensor.accelerometer_xyz().await.unwrap();

    println!("xyz: {}, {}, {}", x, y, z);

    match watchy.get_wakeup_cause() {
        WakeupCause::Reset | WakeupCause::Unknown(_) => {
            println!("reset");

            Circle::new(Point::new(10, 10), 120)
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(&mut watchy.draw_buffer)
                .unwrap_infallible();

            let mut t = ArrayString::<5>::new();
            write!(&mut t, "{:02}:{:02}", time.hour(), time.minute()).unwrap();

            Text::with_baseline(
                t.as_str(),
                Point::new(4, 200 - 20),
                MonoTextStyle::new(
                    &embedded_graphics::mono_font::ascii::FONT_10X20,
                    BinaryColor::On,
                ),
                embedded_graphics::text::Baseline::Top,
            )
            .draw(&mut watchy.draw_buffer)
            .unwrap_infallible();

            Text::with_baseline(
                "test",
                Point::new(50, 200 - 20),
                MonoTextStyle::new(
                    &embedded_graphics::mono_font::ascii::FONT_10X20,
                    BinaryColor::On,
                ),
                embedded_graphics::text::Baseline::Top,
            )
            .draw(&mut watchy.draw_buffer)
            .unwrap_infallible();

            println!("time: {}", esp_hal::time::now());

            watchy.draw_buffer_to_display().await.unwrap();
        }

        WakeupCause::ExternalRtcAlarm => {
            println!("RTC alarm")
        }

        WakeupCause::ButtonPress(_) => {
            println!("button pressed");

            Rectangle::new(Point::new(10, 10), Size::new(180, 180))
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
                .draw(&mut watchy.draw_buffer)
                .unwrap_infallible();

            Triangle::new(Point::new(0, 0), Point::new(5, 5), Point::new(0, 10))
                .translate(Point::new(16, 18))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(&mut watchy.draw_buffer)
                .unwrap_infallible();

            Text::with_baseline(
                "ayy lmao",
                Point::new(24, 14),
                MonoTextStyle::new(
                    &embedded_graphics::mono_font::ascii::FONT_9X18_BOLD,
                    BinaryColor::On,
                ),
                embedded_graphics::text::Baseline::Top,
            )
            .draw(&mut watchy.draw_buffer)
            .unwrap_infallible();

            watchy.draw_buffer_to_display().await.unwrap();
        }
    }

    watchy
        .external_rtc
        .set_alarm(&pcf8563_async::AlarmConfig {
            minute: Some(if time.minute() >= 59 {
                0
            } else {
                time.minute() + 1
            }),
            ..Default::default()
        })
        .await
        .unwrap();
    watchy.external_rtc.enable_alarm().await.unwrap();

    println!("sleep");

    watchy.sleep_deep()
}
