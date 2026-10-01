#![no_std]
#![no_main]

use core::fmt::Write as _;

use arrayvec::ArrayString;
use bma423_async::SensorPower;
use embassy_executor::Spawner;
use embedded_graphics::{
    geometry::Point,
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::BinaryColor,
    text::Text,
    Drawable,
};
use esp_backtrace as _;
use esp_println::{self as _, println};

use crate::{
    display::Display,
    draw_buffer::DrawBuffer,
    watchy::{WakeupCause, Watchy},
};

mod battery;
mod display;
mod draw_buffer;
mod vibration_motor;
mod watchy;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let mut watchy = Watchy::init().expect("watchy init");

    let cause = watchy.wakeup_cause();

    println!("{:?}", cause);

    println!("voltage: {}", watchy.battery.voltage().await);

    if cause == WakeupCause::Reset {
        watchy.external_rtc.reset().await.expect("RTC reset");
        watchy
            .sensor
            .initialize()
            .await
            .expect("initialize accelerometer");
        watchy
            .sensor
            .toggle_sensors(SensorPower::ACCELEROMETER)
            .await
            .expect("toggle accelerometer");
    }

    println!(
        "enabled sensors: {:?}",
        watchy.sensor.enabled_sensors().await
    );
    println!(
        "accelerometer: {:?}",
        watchy.sensor.accelerometer_xyz().await
    );

    let (hour, minute) = watchy
        .external_rtc
        .read_time()
        .await
        .map_or((0, 0), |t| (t.hour(), t.minute()));

    draw_clock(&mut watchy.draw_buffer, hour, minute);

    watchy
        .display
        .draw(watchy.draw_buffer.as_array(), cause == WakeupCause::Reset)
        .await
        .unwrap();

    watchy.display.hibernate().await.unwrap();

    println!("sleeping");

    watchy.hibernate()
}

fn draw_clock(buffer: &mut DrawBuffer, hour: u8, minute: u8) {
    let mut text = ArrayString::<5>::new();
    write!(&mut text, "{hour:02}:{minute:02}").expect("write time to buffer");

    let style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);

    let origin = Point::new(
        (Display::WIDTH as i32 - 50) / 2,
        (Display::HEIGHT as i32 - 20) / 2,
    );

    Text::with_baseline(
        text.as_str(),
        origin,
        style,
        embedded_graphics::text::Baseline::Top,
    )
    .draw(buffer)
    .unwrap();
}
