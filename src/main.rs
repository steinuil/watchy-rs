#![no_std]
#![no_main]

use core::fmt::Write as _;

use arrayvec::ArrayString;
use embassy_executor::Spawner;
use embedded_graphics::{
    geometry::Point, mono_font::MonoTextStyle, pixelcolor::BinaryColor, text::Text, Drawable,
};
use esp_backtrace as _;
use esp_println::{self as _, println};

use crate::{
    draw_buffer::DrawBuffer,
    scale_target::ScaleTarget,
    upheaval::UPHEAVAL_9,
    watchy::{WakeupCause, Watchy},
};

mod battery;
mod display;
mod draw_buffer;
mod scale_target;
mod upheaval;
mod vibration_motor;
mod watchy;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let mut watchy = Watchy::init().expect("watchy init");

    let cause = watchy.wakeup_cause();

    let time_res = watchy.external_rtc.read_time().await;

    let time = match time_res {
        Ok(t) if cause != WakeupCause::Reset => t,
        Ok(_) | Err(_) => {
            let date_time = time::macros::datetime!(2026-10-01 18:19:00 +2);

            watchy.external_rtc.reset().await.unwrap();
            watchy
                .external_rtc
                .set_date(date_time.date())
                .await
                .unwrap();
            watchy
                .external_rtc
                .set_time(date_time.time())
                .await
                .unwrap();

            date_time.time()
        }
    };

    if cause == WakeupCause::Reset {
        watchy.sensor.initialize().await.unwrap();

        watchy
            .external_rtc
            .set_timer(pcf8563_async::TimerFrequency::_1_60thHz, 1)
            .await
            .unwrap();

        watchy.external_rtc.enable_timer_interrupt().await.unwrap();
    }

    println!("time: {:?}", time);

    watchy.external_rtc.clear_timer_flag().await.unwrap();

    let o_clock = cause == WakeupCause::ExternalRtcAlarm && time.minute() == 0;

    if cause == WakeupCause::Reset || o_clock {
        draw_clock(&mut watchy.draw_buffer, time.hour(), time.minute());
        watchy
            .display
            .draw_full(watchy.draw_buffer.as_array())
            .await
            .unwrap();
    } else {
        draw_clock(&mut watchy.draw_buffer, time.hour(), time.minute());
        watchy
            .display
            .draw_partial(watchy.draw_buffer.as_array())
            .await
            .unwrap();
    }

    watchy.display.hibernate().await.unwrap();

    println!("sleeping");

    watchy.hibernate()
}

fn draw_clock(buffer: &mut DrawBuffer, hour: u8, minute: u8) {
    buffer.clear();

    // let mut text = ArrayString::<5>::new();
    // write!(&mut text, "{hour:02}:{minute:02}").expect("write time to buffer");

    let style = MonoTextStyle::new(&UPHEAVAL_9, BinaryColor::On);

    let scale = 8;

    let mut big = ScaleTarget::new(buffer, scale as u32);

    let mut hours = ArrayString::<2>::new();
    write!(&mut hours, "{hour:02}").expect("write hour to buffer");

    Text::new(hours.as_str(), Point::new(1, 11), style)
        .draw(&mut big)
        .unwrap();

    let mut minutes = ArrayString::<2>::new();
    write!(&mut minutes, "{minute:02}").expect("write minute to buffer");

    Text::new(minutes.as_str(), Point::new(1, 22), style)
        .draw(&mut big)
        .unwrap();
}
