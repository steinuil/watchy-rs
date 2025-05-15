#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod font;
mod watchy;

use arrayvec::ArrayString;
use core::fmt::Write;
use embassy_executor::Spawner;
use embassy_time::Duration;
use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Ellipse, PrimitiveStyle, Rectangle},
    text::Text,
};
use esp_backtrace as _;
use esp_hal::{
    clock::ClockControl,
    embassy,
    peripherals::Peripherals,
    prelude::*,
    spi::{master::Spi, SpiMode},
    timer::TimerGroup,
    Rtc, IO,
};
use esp_println::println;
use watchy::VibrationMotor;

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    embassy::init(&clocks, timer_group0);

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let pins = watchy::Pins::new(io.pins);

    let mut i2c = watchy::init_i2c(peripherals.I2C0, pins.i2c, &clocks);

    // CHECK: interrupts for I2C/SPI/GPIO should be enabled automatically

    let mut pcf8563 = pcf8563_async::PCF8563::new(pcf8563_async::SLAVE_ADDRESS, &mut i2c);

    let mut vibration_motor = VibrationMotor::new(pins.vibration_motor);

    let spi = Spi::new(peripherals.SPI3, 20_u32.MHz(), SpiMode::Mode0, &clocks)
        .with_sck(pins.spi.sck)
        .with_mosi(pins.spi.mosi)
        .with_cs(pins.spi.cs);

    let mut gdeh0154d67 = gdeh0154d67_async::GDEH0154D67::new(
        spi,
        pins.display.dc,
        pins.display.reset,
        pins.display.busy,
        embassy_time::Delay,
    );

    match watchy::get_wakeup_cause(&peripherals.LPWR) {
        watchy::WakeupCause::Reset => {
            println!("Booted");

            let mut rect = |x: u32, y: u32, w: u32, h: u32, tx: u32, ty: u32| {
                Rectangle::new(Point::new(x as i32, y as i32), Size::new(w, h))
                    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                    .translate(Point::new(tx as i32, ty as i32))
                    .draw(&mut gdeh0154d67)
                    .unwrap();
            };

            let w: u32 = 11;
            let h: u32 = 11;
            let ws: u32 = 4;
            let hs: u32 = 4;
            // let w_1: u32 = 17;
            let tx = 0;
            let ty = 76;

            rect(0, 0, w * 3 + ws * 2, h, tx, ty);
            rect(w * 2 + ws * 2, h, w, hs, tx, ty);
            rect(0, h + hs, w * 3 + ws * 2, h, tx, ty);
            rect(0, h * 2 + hs, w, hs, tx, ty);
            rect(0, h * 2 + hs * 2, w * 3 + ws * 2, h, tx, ty);

            let tx = w * 3 + ws * 3;
            // rect(0, 0, w + w_1, h, tx, ty);
            // rect(w_1, h, w, h * 2 + hs * 2, tx, ty);
            rect(0, 0, w, h * 3 + hs * 2, tx, ty);

            // let tx = tx + w + w_1 + ws;
            let tx = tx + w + ws;
            rect(0, h + hs, w, h, tx, ty);
            rect(0, h * 2 + hs * 2, w, h, tx, ty);

            let tx = tx + w + ws;
            rect(0, 0, w * 3 + ws * 2, h, tx, ty);
            rect(w * 2 + ws * 2, h, w, hs, tx, ty);
            rect(0, h + hs, w * 3 + ws * 2, h, tx, ty);
            rect(w * 2 + ws * 2, h * 2 + hs, w, hs, tx, ty);
            rect(0, h * 2 + hs * 2, w * 3 + ws * 2, h, tx, ty);

            let tx = tx + w * 3 + ws * 3;
            rect(0, 0, w, h * 2 + hs, tx, ty);
            rect(w, h + hs, w * 2 + ws * 2, h, tx, ty);
            rect(w * 2 + ws * 2, 0, w, h * 3 + hs * 2, tx, ty);

            // let time = pcf8563.read_time().await.unwrap();
            // let mut t = ArrayString::<5>::new();
            // write!(&mut t, "{:02}:{:02}", time.hour(), time.minute()).unwrap();

            gdeh0154d67.draw2(true).await.unwrap();
        }
        watchy::WakeupCause::ExternalRtcAlarm => {
            println!("RTC alarm");

            match pcf8563.read_time().await {
                Ok(time) if time.minute() == 0 => {
                    vibration_motor
                        .vibrate_linear(2, Duration::from_millis(75))
                        .await;
                }
                Ok(_) | Err(_) => {}
            }
        }
        watchy::WakeupCause::ButtonPress(watchy::Button::BottomLeft) => {
            Ellipse::new(Point::new(20, 20), Size::new(160, 160))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(&mut gdeh0154d67)
                .unwrap();

            let time = pcf8563.read_time().await.unwrap();
            let mut t = ArrayString::<5>::new();
            write!(&mut t, "{:02}:{:02}", time.hour(), time.minute()).unwrap();
            Text::with_baseline(
                t.as_str(),
                Point::new(4, 200 - 15 - 4),
                MonoTextStyle::new(
                    &embedded_graphics::mono_font::ascii::FONT_9X15,
                    BinaryColor::On,
                ),
                embedded_graphics::text::Baseline::Top,
            )
            .draw(&mut gdeh0154d67)
            .unwrap();

            println!("initialize");
            gdeh0154d67.draw2(false).await.unwrap();
        }
        watchy::WakeupCause::ButtonPress(button) => {
            println!("Button pressed: {:?}", button);
        }
        watchy::WakeupCause::UnknownExt1(mask) => {
            println!("Unknown EXT1 mask: {}", mask);
        }
        watchy::WakeupCause::Unknown(source) => {
            println!("Unknown source: {:?}", source);
        }
    }

    {
        // match pcf8563.read_datetime().await {
        //     Ok(time) => println!("time: {}", time),
        //     Err(e) => println!("error reading time: {:?}", e),
        // }

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

    let rtc = Rtc::new(peripherals.LPWR);
    let delay = esp32_hal::Delay::new(&clocks);
    println!("sleepy");
    watchy::sleep_deep(rtc, delay, pins.external_rtc, pins.buttons);
}
