#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![allow(clippy::empty_loop)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp32_hal::{
    clock::ClockControl,
    embassy,
    peripherals::{Peripherals, RTC_CNTL},
    prelude::*,
    reset::SleepSource,
    rtc_cntl::sleep::{Ext0WakeupSource, Ext1WakeupSource, WakeupLevel},
    timer::TimerGroup,
    Delay, IO,
};
use esp_backtrace as _;
use esp_println::println;

#[embassy_executor::task]
async fn run1() {
    loop {
        println!("ayy");
        Timer::after(Duration::from_millis(1000)).await;
    }
}

#[embassy_executor::task]
async fn run2() {
    loop {
        println!("lmao");
        Timer::after(Duration::from_millis(2000)).await;
    }
}

const RTCIO_GPIO4_CHANNEL: u32 = 1 << 10;
const RTCIO_GPIO25_CHANNEL: u32 = 1 << 6;
const RTCIO_GPIO26_CHANNEL: u32 = 1 << 7;
const RTCIO_GPIO35_CHANNEL: u32 = 1 << 5;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum Button {
    /// Bottom left button
    Menu,
    /// Top left button
    Back,
    /// Top right button
    Up,
    /// Bottom right button
    Down,
}

fn get_ext1_wakeup_button(rtc_cntl: RTC_CNTL) -> Result<Button, u32> {
    let wakeup_bits = rtc_cntl.ext_wakeup1_status.read().bits();

    match wakeup_bits {
        RTCIO_GPIO26_CHANNEL => Ok(Button::Menu),
        RTCIO_GPIO25_CHANNEL => Ok(Button::Back),
        RTCIO_GPIO35_CHANNEL => Ok(Button::Up),
        RTCIO_GPIO4_CHANNEL => Ok(Button::Down),
        _ => Err(wakeup_bits),
    }
}

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);

    let mut rtc = esp32_hal::Rtc::new(peripherals.RTC_CNTL);

    let mut io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    embassy::init(&clocks, timer_group0.timer0);

    let cause = esp32_hal::reset::get_wakeup_cause();

    match cause {
        // RTC alarm
        SleepSource::Ext0 => {
            println!("RTC alarm (display needs to be updated)");
        }
        // Button press
        SleepSource::Ext1 => match get_ext1_wakeup_button(peripherals.RTC_CNTL) {
            Ok(Button::Menu) => {
                println!("Menu button pressed");
            }
            Ok(Button::Back) => {
                println!("Down button pressed");
            }
            Ok(Button::Up) => {
                println!("Up button pressed");
            }
            Ok(Button::Down) => {
                println!("Down button pressed");
            }
            Err(wakeup_status) => {
                println!("wakeup_status bitmask not recognized: {}", wakeup_status);
            }
        },
        // Booted
        SleepSource::Undefined => {
            println!("Booted");
        }
        _ => {
            println!("unknown wakeup cause: {:?}", cause);
        }
    }

    let mut delay = Delay::new(&clocks);

    rtc.sleep_deep(
        &[
            // should be low according to the C code
            // &Ext0WakeupSource::new(&mut io.pins.gpio27, WakeupLevel::Low),
            &Ext1WakeupSource::new(
                &mut [
                    // Menu button
                    &mut io.pins.gpio26,
                    // Back button
                    &mut io.pins.gpio25,
                    // Down button
                    &mut io.pins.gpio4,
                    // Up button
                    &mut io.pins.gpio35,
                ],
                WakeupLevel::High,
            ),
        ],
        &mut delay,
    );
}
