#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![allow(clippy::empty_loop)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp32_hal::{
    clock::ClockControl,
    embassy,
    i2c::I2C,
    peripherals::{Interrupt, Peripherals, RTC_CNTL},
    prelude::*,
    reset::SleepSource,
    rtc_cntl::sleep::{Ext0WakeupSource, Ext1WakeupSource, WakeupLevel},
    timer::TimerGroup,
    Delay, Rtc, IO,
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
    BottomLeft,
    TopLeft,
    TopRight,
    BottomRight,
}

fn get_ext1_wakeup_button(_rtc: &Rtc) -> Result<Button, u32> {
    let wakeup_bits = unsafe { (*RTC_CNTL::PTR).ext_wakeup1_status.read() }.bits();

    match wakeup_bits {
        RTCIO_GPIO26_CHANNEL => Ok(Button::BottomLeft),
        RTCIO_GPIO25_CHANNEL => Ok(Button::TopLeft),
        RTCIO_GPIO35_CHANNEL => Ok(Button::TopRight),
        RTCIO_GPIO4_CHANNEL => Ok(Button::BottomRight),
        _ => Err(wakeup_bits),
    }
}

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    embassy::init(&clocks, timer_group0.timer0);

    let mut io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let mut i2c = I2C::new(
        peripherals.I2C0,
        io.pins.gpio21,
        io.pins.gpio22,
        400u32.kHz(),
        &clocks,
    );

    // Interrupts need to be enabled for i2c to work
    esp32_hal::interrupt::enable(
        Interrupt::I2C_EXT0,
        esp32_hal::interrupt::Priority::Priority1,
    )
    .unwrap();

    let mut delay = Delay::new(&clocks);

    let mut pcf8563 = pcf8563_async::PCF8563::new(pcf8563_async::SLAVE_ADDRESS, &mut i2c);

    // let mut bma423 = bma423_async::BMA423::new(0x18, &mut i2c, &mut delay);

    match pcf8563.read_datetime().await {
        Ok(time) => println!("time: {}", time),
        Err(e) => println!("error reading time: {:?}", e),
    }

    let mut rtc = Rtc::new(peripherals.RTC_CNTL);

    let cause = esp32_hal::reset::get_wakeup_cause();

    match cause {
        // RTC alarm
        SleepSource::Ext0 => {
            println!("RTC alarm (display needs to be updated)");
        }
        // Button press
        SleepSource::Ext1 => match get_ext1_wakeup_button(&rtc) {
            Ok(Button::BottomLeft) => {
                println!("Menu button pressed");
            }
            Ok(Button::TopLeft) => {
                println!("Back button pressed");
            }
            Ok(Button::TopRight) => {
                println!("Up button pressed");
            }
            Ok(Button::BottomRight) => {
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

    let minute = pcf8563.read_time().await.unwrap().minute();
    pcf8563
        .set_alarm_interrupt(&pcf8563_async::AlarmConfig {
            minute: Some(if minute >= 59 { 0 } else { minute + 1 }),
            ..Default::default()
        })
        .await
        .unwrap();
    pcf8563.enable_alarm_interrupt().await.unwrap();

    rtc.sleep_deep(
        &[
            // should be low according to the C code
            &Ext0WakeupSource::new(&mut io.pins.gpio27, WakeupLevel::Low),
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
