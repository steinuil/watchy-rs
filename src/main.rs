#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod watchy;

use embassy_executor::Spawner;
use embedded_hal_async::delay::DelayNs;
use esp32_hal::{
    clock::ClockControl,
    dma::DmaPriority,
    embassy,
    pdma::Dma,
    peripherals::Peripherals,
    prelude::*,
    spi::{
        master::{dma::WithDmaSpi3, Spi},
        SpiMode,
    },
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

    let dma = Dma::new(system.dma);

    let (tx_buffer, mut tx_descriptors, _, mut rx_descriptors) = esp32_hal::dma_buffers!(6000, 0);

    let mut spi = Spi::new(peripherals.SPI3, 20_u32.MHz(), SpiMode::Mode0, &clocks)
        .with_sck(sck_pin)
        .with_mosi(mosi_pin)
        .with_cs(cs_pin)
        .with_dma(dma.spi3channel.configure(
            false,
            &mut tx_descriptors,
            &mut rx_descriptors,
            DmaPriority::Priority0,
        ));

    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;
    rst_pin.set_low().unwrap();
    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;
    rst_pin.set_high().unwrap();
    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;
    println!("HW reset done");

    dc_pin.set_low().unwrap();
    println!("DC pin set low");
    tx_buffer[0] = 0x012;
    embedded_hal::spi::SpiBus::write(&mut spi, &tx_buffer[..1]).unwrap();
    println!("written SW_RESET");
    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;
    println!("SW reset done");

    dc_pin.set_low().unwrap();
    println!("DC pin set low");
    tx_buffer[0] = 0x01;
    embedded_hal::spi::SpiBus::write(&mut spi, &tx_buffer[..1]).unwrap();
    dc_pin.set_high().unwrap();
    println!("DC pin set high");
    tx_buffer[0] = 0xc7;
    tx_buffer[1] = 0b0;
    tx_buffer[2] = 0x00;
    embedded_hal::spi::SpiBus::write(&mut spi, &tx_buffer[..3]).unwrap();
    println!("set driver control output")
}
