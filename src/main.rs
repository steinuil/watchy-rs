#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod watchy;

use arrayvec::ArrayString;
use core::fmt::Write;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Ellipse, PrimitiveStyle},
    text::Text,
};
use embedded_hal::spi::SpiDevice;
use embedded_hal_async::delay::DelayNs;
use esp32_hal::{
    clock::ClockControl,
    dma::DmaPriority,
    embassy,
    i2c::I2C,
    pdma::Dma,
    peripherals::{Interrupt, Peripherals},
    prelude::*,
    rtc_cntl::sleep::{Ext0WakeupSource, Ext1WakeupSource, WakeupLevel},
    spi::{
        master::{dma::WithDmaSpi3, Spi},
        SpiMode,
    },
    timer::TimerGroup,
    FlashSafeDma, Rtc, IO,
};
use esp_backtrace as _;
use esp_println::println;
use watchy::VibrationMotor;

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    embassy::init(&clocks, timer_group0.timer0);

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let mut pins = watchy::Pins::new(io.pins);

    let mut i2c = watchy::init_i2c(peripherals.I2C0, pins.i2c, &clocks);

    // CHECK: interrupts for I2C/SPI/GPIO should be enabled automatically

    let mut pcf8563 = pcf8563_async::PCF8563::new(pcf8563_async::SLAVE_ADDRESS, &mut i2c);

    let mut vibration_motor = VibrationMotor::new(pins.vibration_motor);

    // - FlashSafeDma doesn't work because the arrays need to be placed in ROM.
    // - Interrupts are enabled automagically by embassy::init

    // CHECK: is this correct maybe we need to directly pass these to write in
    // actually no because it doesn't make sense, these are &mut borrowed later
    // so we can't modify them
    let (tx_buffer, mut tx_descriptors, _, mut rx_descriptors) = esp32_hal::dma_buffers!(6000, 0);

    let dma = Dma::new(system.dma);

    let spi = Spi::new(peripherals.SPI3, 20_u32.MHz(), SpiMode::Mode0, &clocks)
        .with_sck(pins.spi.sck)
        .with_mosi(pins.spi.mosi)
        .with_cs(pins.spi.cs)
        .with_dma(dma.spi3channel.configure(
            false,
            &mut tx_descriptors,
            &mut rx_descriptors,
            DmaPriority::Priority0,
        ));

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
        }
        watchy::WakeupCause::ExternalRtcAlarm => {
            println!("RTC alarm");

            // vibration_motor
            //     .vibrate_linear(2, Duration::from_millis(75))
            //     .await;
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

            println!("lmao");
            gdeh0154d67.initialize().await.unwrap();
            println!("tfw");
            gdeh0154d67.draw().await.unwrap();
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
    watchy::sleep_deep(rtc, delay, pins.external_rtc, pins.buttons);
}
