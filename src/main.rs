#![no_std]
#![no_main]

use core::fmt::Write as _;

use arrayvec::ArrayString;
use embassy_executor::Spawner;
// use embassy_time::Delay;
use embedded_graphics::{
    geometry::Point,
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::BinaryColor,
    text::Text,
    Drawable,
};
// use embedded_hal_async::delay::DelayNs as _;
use esp_backtrace as _;
use esp_hal::{
    gpio::{Event, Input, InputConfig, Pull, WakeupConfig},
    i2c,
    rtc_cntl::{sleep::LowPower, WakeupSource},
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_println::{self as _, println};
use pcf8563_async::PCF8563;

use crate::{display::Display, draw_buffer::DrawBuffer};

mod battery;
pub mod display;
mod draw_buffer;
mod vibration_motor;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let peripherals = esp_hal::init(esp_hal::Config::default());

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    let i2c = i2c::master::I2c::new(
        peripherals.I2C0,
        i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .expect("i2c initialization")
    .with_sda(peripherals.GPIO21)
    .with_scl(peripherals.GPIO22)
    .into_async();

    let mut clock = PCF8563::new(pcf8563_async::SLAVE_ADDRESS, i2c);

    let mut display = display::Display::new(
        peripherals.SPI3,
        peripherals.DMA_SPI3,
        peripherals.GPIO18,
        peripherals.GPIO23,
        peripherals.GPIO5,
        peripherals.GPIO10,
        peripherals.GPIO9,
        peripherals.GPIO19,
    )
    .expect("display initialization");

    let cause = esp_hal::rtc_cntl::wakeup_cause();

    if cause.contains(WakeupSource::Ext0) {
        clock.clear_alarm_flag().await.expect("clear alarm");
    }

    println!("{:?}", cause);

    let (hour, minute) = clock
        .read_time()
        .await
        .map_or((0, 0), |t| (t.hour(), t.minute()));

    let mut buffer = DrawBuffer::empty();
    draw_clock(&mut buffer, hour, minute);

    if cause.is_empty() {
        display.clear(ssd1681_async::BwPixel::White).await.unwrap();
        display.write_frame(buffer.as_array()).await.unwrap();
        display.refresh_full().await.unwrap()
    } else {
        display.write_frame(buffer.as_array()).await.unwrap();
        display.refresh_partial().await.unwrap();
    }

    display.hibernate().await.unwrap();

    let mut rtc_int = Input::new(
        peripherals.GPIO27,
        InputConfig::default().with_pull(Pull::Up),
    );
    rtc_int.listen(Event::LowLevel);
    rtc_int
        .apply_wakeup_config(&WakeupConfig::default().with_low_power_path(true))
        .unwrap();

    let mut btn = Input::new(
        peripherals.GPIO26,
        InputConfig::default().with_pull(Pull::Down),
    );
    btn.listen(Event::HighLevel);
    btn.apply_wakeup_config(&WakeupConfig::default().with_low_power_path(true))
        .unwrap();

    let mut lwpr = LowPower::new(peripherals.LPWR);
    lwpr.sleep_deep(esp_hal::rtc_cntl::sleep::RtcSleepConfig::deep())
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
