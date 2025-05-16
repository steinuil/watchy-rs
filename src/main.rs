#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use defmt::println;
use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal_embassy::main;
use watchy::{WakeupCause, Watchy};

mod buttons;
mod font;
mod vibration_motor;
mod watchy;

#[main]
async fn main(_spawner: Spawner) {
    let mut watchy = match Watchy::init() {
        Ok(watchy) => watchy,
        Err(error) => {
            println!("{:?}", error);
            return;
        }
    };

    match watchy.get_wakeup_cause() {
        WakeupCause::Reset | WakeupCause::Unknown(_) => {
            println!("reset");
        }

        WakeupCause::ExternalRtcAlarm => {
            println!("RTC alarm")
        }

        WakeupCause::ButtonPress(_) => {
            println!("button pressed")
        }
    }

    let minute = watchy.external_rtc.read_time().await.unwrap().minute();
    watchy
        .external_rtc
        .set_alarm_interrupt(&pcf8563_async::AlarmConfig {
            minute: Some(if minute >= 59 { 0 } else { minute + 1 }),
            ..Default::default()
        })
        .await
        .unwrap();
    watchy.external_rtc.enable_alarm_interrupt().await.unwrap();

    watchy.sleep_deep()
}
