#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod watchy;

use embassy_executor::Spawner;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::SpiDevice;
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
    FlashSafeDma, IO,
};
use esp_backtrace as _;
use esp_println::println;

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    embassy::init(&clocks, timer_group0.timer0);

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let sck_pin = io.pins.gpio18;
    let mosi_pin = io.pins.gpio23;
    let cs_pin = io.pins.gpio5.into_push_pull_output();

    let mut dc_pin = io.pins.gpio10.into_push_pull_output();
    let mut rst_pin = io.pins.gpio9.into_push_pull_output();

    let (mut tx_descriptors, mut rx_descriptors) = esp32_hal::dma_descriptors!(6000);

    let dma = Dma::new(system.dma);

    let spi = Spi::new(peripherals.SPI3, 20_u32.MHz(), SpiMode::Mode0, &clocks)
        .with_sck(sck_pin)
        .with_mosi(mosi_pin)
        .with_dma(dma.spi3channel.configure(
            false,
            &mut tx_descriptors,
            &mut rx_descriptors,
            DmaPriority::Priority0,
        ));

    let spi = FlashSafeDma::<_, 6000>::new(spi);

    let mut spi = embedded_hal_bus::spi::ExclusiveDevice::new(spi, cs_pin, embassy_time::Delay);

    // HW reset
    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;
    rst_pin.set_low().unwrap();
    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;
    rst_pin.set_high().unwrap();
    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;

    println!("HW reset done");

    // SW reset
    dc_pin.set_low().unwrap();
    println!("DC pin set low");
    spi.write(&[0x12]).await.unwrap();
    println!("written SW_RESET");
    DelayNs::delay_ms(&mut embassy_time::Delay, 10).await;

    println!("SW reset done");
}
