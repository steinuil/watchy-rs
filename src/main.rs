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
    pdma::{Dma, Spi3DmaChannel},
    peripherals::{Interrupt, Peripherals, RTC_CNTL},
    prelude::*,
    reset::SleepSource,
    rtc_cntl::sleep::{Ext0WakeupSource, Ext1WakeupSource, WakeupLevel},
    spi::master::{prelude::*, Spi, SpiBusController, SpiBusDevice},
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
    // TODO when esp32_hal lets you read the wakeup status, it'd be nice to use that
    // instead of using unsafe.
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

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let sda_pin = io.pins.gpio21;
    let scl_pin = io.pins.gpio22;

    let sck_pin = io.pins.gpio18;
    let mosi_pin = io.pins.gpio23;
    let cs_pin = io.pins.gpio5;

    let dc_pin = io.pins.gpio10;
    let reset_pin = io.pins.gpio9;
    let busy_pin = io.pins.gpio19;

    let vibration_motor_pin = io.pins.gpio13;

    let mut rtc_interrupt_pin = io.pins.gpio27;

    let mut button_bottom_left_pin = io.pins.gpio26;
    let mut button_top_left_pin = io.pins.gpio25;
    let mut button_top_right_pin = io.pins.gpio35;
    let mut button_bottom_right_pin = io.pins.gpio4;

    // TODO: maybe use an embassy_sync::mutex::Mutex to share the i2c bus
    // between the devices
    let mut i2c = I2C::new(peripherals.I2C0, sda_pin, scl_pin, 400u32.kHz(), &clocks);

    // Interrupts need to be enabled for i2c to work
    esp32_hal::interrupt::enable(
        Interrupt::I2C_EXT0,
        esp32_hal::interrupt::Priority::Priority1,
    )
    .unwrap();

    let dma = Dma::new(system.dma);

    let mut tx_descr = [0u32; 3];
    let mut rx_descr = [0u32; 3];

    let spi = Spi::new_no_miso(
        peripherals.SPI3,
        sck_pin,
        mosi_pin,
        cs_pin,
        20u32.MHz(),
        esp32_hal::spi::SpiMode::Mode0,
        &clocks,
    )
    .with_dma(dma.spi3channel.configure(
        false,
        &mut tx_descr,
        &mut rx_descr,
        esp32_hal::dma::DmaPriority::Priority0,
    ));

    esp32_hal::interrupt::enable(
        Interrupt::SPI3_DMA,
        esp32_hal::interrupt::Priority::Priority1,
    )
    .unwrap();

    let mut rtc = Rtc::new(peripherals.RTC_CNTL);

    let cause = esp32_hal::reset::get_wakeup_cause();

    match cause {
        // RTC alarm
        SleepSource::Ext0 => {
            println!("RTC alarm (display needs to be updated)");

            let mut vib = vibration_motor_pin.into_push_pull_output();

            let mut motor_on = false;
            for _ in 0..4 {
                motor_on = !motor_on;
                println!("motor_on = {}", motor_on);
                vib.set_output_high(motor_on);
                Timer::after(Duration::from_millis(75)).await
            }
        }
        // Button press
        SleepSource::Ext1 => match get_ext1_wakeup_button(&rtc) {
            Ok(Button::BottomLeft) => {
                println!("Menu button pressed");

                {
                    let mut gdeh0154d67 = gdeh0154d67_async::GDEH0154D67::new(
                        spi,
                        dc_pin.into_push_pull_output(),
                        reset_pin.into_push_pull_output(),
                        busy_pin.into_pull_up_input(),
                        embassy_time::Delay,
                    );

                    println!("display acquired");

                    gdeh0154d67.initialize().await.unwrap();
                    println!("display initialized");
                    gdeh0154d67
                        .draw(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
                        .await
                        .unwrap();
                    println!("drawn");
                }
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

    {
        let mut pcf8563 = pcf8563_async::PCF8563::new(pcf8563_async::SLAVE_ADDRESS, &mut i2c);

        match pcf8563.read_datetime().await {
            Ok(time) => println!("time: {}", time),
            Err(e) => println!("error reading time: {:?}", e),
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
    }

    let mut delay = Delay::new(&clocks);

    rtc.sleep_deep(
        &[
            // should be low according to the C code
            &Ext0WakeupSource::new(&mut rtc_interrupt_pin, WakeupLevel::Low),
            &Ext1WakeupSource::new(
                &mut [
                    &mut button_bottom_left_pin,
                    &mut button_top_left_pin,
                    &mut button_top_right_pin,
                    &mut button_bottom_right_pin,
                ],
                WakeupLevel::High,
            ),
        ],
        &mut delay,
    );
}
