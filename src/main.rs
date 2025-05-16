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
    prelude::{Point, Primitive},
    primitives::{Circle, PrimitiveStyle},
    text::Text,
    Drawable as _,
};
use esp_backtrace as _;
use esp_hal_embassy::main;
use esp_println as _;
use watchy::{WakeupCause, Watchy};

mod buttons;
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

    let time = watchy.external_rtc.read_time().await.unwrap();

    println!("battery voltage: {}", watchy.battery.voltage());

    match watchy.get_wakeup_cause() {
        WakeupCause::Reset | WakeupCause::Unknown(_) => {
            println!("reset");

            Circle::new(Point::new(10, 10), 120)
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(&mut watchy.display)
                .unwrap();

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
            .draw(&mut watchy.display)
            .unwrap();

            watchy.display.draw2(true).await.unwrap();
        }

        WakeupCause::ExternalRtcAlarm => {
            println!("RTC alarm")
        }

        WakeupCause::ButtonPress(_) => {
            println!("button pressed")
        }
    }

    watchy
        .external_rtc
        .set_alarm_interrupt(&pcf8563_async::AlarmConfig {
            minute: Some(if time.minute() >= 59 {
                0
            } else {
                time.minute() + 1
            }),
            ..Default::default()
        })
        .await
        .unwrap();
    watchy.external_rtc.enable_alarm_interrupt().await.unwrap();

    println!("sleep");

    watchy.sleep_deep()
}
