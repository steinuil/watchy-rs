#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod watchy;

use embassy_executor::Spawner;
use embedded_hal_async::delay::DelayNs;
use esp32_hal::{
    clock::ClockControl,
    embassy,
    peripherals::Peripherals,
    prelude::*,
    spi::{master::Spi, SpiMode},
    timer::TimerGroup,
    IO,
};
use esp_backtrace as _;
use esp_println::println;

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    embassy::init(&clocks, timer_group0);

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let sck_pin = io.pins.gpio18;
    let mosi_pin = io.pins.gpio23;
    let cs_pin = io.pins.gpio5.into_push_pull_output();

    let mut dc_pin = io.pins.gpio10.into_push_pull_output();
    let mut rst_pin = io.pins.gpio9.into_push_pull_output();

    let mut spi = Spi::new(peripherals.SPI3, 20_u32.MHz(), SpiMode::Mode0, &clocks)
        .with_sck(sck_pin)
        .with_mosi(mosi_pin)
        .with_cs(cs_pin);

    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;
    rst_pin.set_low().unwrap();
    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;
    rst_pin.set_high().unwrap();
    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;
    println!("HW reset done");

    dc_pin.set_low().unwrap();
    println!("DC pin set low");
    spi.write(&[0x12]).unwrap();
    println!("written SW_RESET");
    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;
    println!("SW reset done");

    dc_pin.set_low().unwrap();
    println!("DC pin set low");
    spi.write(&[0x01]).unwrap();
    dc_pin.set_high().unwrap();
    println!("DC pin set high");
    spi.write(&[0xc7, 0b0, 0x00]).unwrap();
    println!("set driver control output")
}
