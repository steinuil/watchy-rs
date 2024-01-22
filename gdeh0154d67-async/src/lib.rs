// Currently not very async because esp32-hal does not have an impl
// for embedded_hal_async::SpiDevice, and right now when you try to pass
// an SpiBus and use the async traits it hangs when trying to write on the bus.

#![no_std]

use core::convert::Infallible;

use embedded_graphics_core::{
    pixelcolor::BinaryColor,
    prelude::{DrawTarget, OriginDimensions},
    Pixel,
};
use embedded_hal::{
    digital::{InputPin, OutputPin},
    spi::SpiBus,
};
use embedded_hal_async::{delay::DelayNs, digital::Wait};

#[derive(Debug)]
pub enum Error<E> {
    Spi(E),
}

impl<E: core::fmt::Display> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Spi(e) => write!(f, "Bus error: {}", e),
        }
    }
}

impl<E> From<E> for Error<E> {
    fn from(err: E) -> Self {
        Error::Spi(err)
    }
}

mod command {
    pub const DRIVER_OUTPUT_CONTROL: u8 = 0x01;
    pub const DEEP_SLEEP_MODE: u8 = 0x10;
    pub const DATA_ENTRY_MODE_SETTING: u8 = 0x11;
    pub const SW_RESET: u8 = 0x12;
    pub const TEMPERATURE_SENSOR_CONTROL: u8 = 0x18;
    pub const MASTER_ACTIVATION: u8 = 0x20;
    pub const DISPLAY_UPDATE_CONTROL_1: u8 = 0x21;
    pub const DISPLAY_UPDATE_CONTROL_2: u8 = 0x22;
    pub const WRITE_RAM_BW: u8 = 0x24;
    pub const BORDER_WAVEFORM_CONTROL: u8 = 0x3c;
    pub const SET_RAM_X_START_END_POSITION: u8 = 0x44;
    pub const SET_RAM_Y_START_END_POSITION: u8 = 0x45;
    pub const SET_RAM_X_ADDRESS_POSITION: u8 = 0x4e;
    pub const SET_RAM_Y_ADDRESS_POSITION: u8 = 0x4f;
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum TemperatureSensor {
    External = 0x48,
    Internal = 0x80,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum DeepSleepMode {
    Normal = 0b00,
    RetainRAM = 0b01,
    ResetRAM = 0b11,
}

pub struct GDEH0154D67<SPI, DC, RES, Busy, Delay> {
    spi: SPI,
    dc: DC,
    reset: RES,
    busy: Busy,
    delay: Delay,
    buffer: [u8; 200 * 200 / 8],
}

impl<SPI, DC, RES, Busy, Delay, E> GDEH0154D67<SPI, DC, RES, Busy, Delay>
where
    SPI: SpiBus<Error = E>,
    DC: OutputPin<Error = Infallible>,
    RES: OutputPin<Error = Infallible>,
    Busy: InputPin<Error = Infallible> + Wait,
    Delay: DelayNs,
{
    pub fn new(
        spi: SPI,
        data_command_pin: DC,
        reset_pin: RES,
        busy_pin: Busy,
        delay: Delay,
    ) -> Self {
        GDEH0154D67 {
            spi,
            dc: data_command_pin,
            reset: reset_pin,
            busy: busy_pin,
            delay,
            buffer: [0xFF; 200 * 200 / 8],
        }
    }

    pub async fn initialize(&mut self) -> Result<(), Error<E>> {
        self.delay.delay_ms(10).await;
        esp_println::println!("0");
        self.hardware_reset().await?;
        esp_println::println!("1");
        self.software_reset().await?;
        esp_println::println!("2");
        self.set_driver_output().await?;
        esp_println::println!("3");
        self.set_data_entry_mode().await?;
        esp_println::println!("4");
        self.set_ram_x_start_end_position(0, 200).await?;
        esp_println::println!("5");
        self.set_ram_y_start_end_position(0, 200).await?;
        esp_println::println!("6");
        self.set_border_waveform().await?;
        esp_println::println!("7");
        self.set_temperature_sensor(TemperatureSensor::Internal)
            .await?;
        esp_println::println!("8");
        self.set_display_update_sequence(0xb1).await?;
        esp_println::println!("9");
        self.master_activation().await?;
        esp_println::println!("10");
        Ok(())
    }

    pub async fn draw(&mut self) -> Result<(), Error<E>> {
        self.set_ram_x_address_position(0).await?;
        self.set_ram_y_address_position(0).await?;
        // self.write_bw_ram(&self.buffer[..])?;
        self.dc.set_low().unwrap();
        self.spi.write(&[command::WRITE_RAM_BW])?;
        self.dc.set_high().unwrap();
        self.spi.write(&self.buffer[..])?;
        // self.write_command_data(command::WRITE_RAM_BW, self.buffer.as_slice())?;
        self.set_display_update_sequence(0xc7).await?;
        self.master_activation().await?;
        self.set_deep_sleep_mode(DeepSleepMode::ResetRAM).await?;
        Ok(())
    }

    async fn hardware_reset(&mut self) -> Result<(), Error<E>> {
        self.reset.set_low().unwrap();
        self.delay.delay_ms(10).await;
        self.reset.set_high().unwrap();
        self.delay.delay_ms(10).await;
        Ok(())
    }

    async fn software_reset(&mut self) -> Result<(), Error<E>> {
        self.write_command(command::SW_RESET).await?;
        // According to the SSD1681 spec
        self.delay.delay_ms(10).await;
        Ok(())
    }

    async fn set_driver_output(&mut self) -> Result<(), Error<E>> {
        self.write_command_data(command::DRIVER_OUTPUT_CONTROL, &[0xc7, 0b0, 0x00])
            .await?;
        Ok(())
    }

    async fn set_data_entry_mode(&mut self) -> Result<(), Error<E>> {
        self.write_command_data(command::DATA_ENTRY_MODE_SETTING, &[0b0_11])
            .await?;
        Ok(())
    }

    async fn set_ram_x_start_end_position(&mut self, x: u16, width: u16) -> Result<(), Error<E>> {
        self.write_command_data(
            command::SET_RAM_X_START_END_POSITION,
            &[(x / 8) as u8, ((x + width - 1) / 8) as u8],
        )
        .await?;
        Ok(())
    }

    async fn set_ram_y_start_end_position(&mut self, y: u16, height: u16) -> Result<(), Error<E>> {
        self.write_command_data(
            command::SET_RAM_Y_START_END_POSITION,
            &[
                (y % 0xFF) as u8,
                (y / 0xFF) as u8,
                ((y + height - 1) % 0xFF) as u8,
                ((y + height - 1) / 0xFF) as u8,
            ],
        )
        .await?;
        Ok(())
    }

    async fn set_ram_x_address_position(&mut self, x: u16) -> Result<(), Error<E>> {
        self.write_command_data(command::SET_RAM_X_ADDRESS_POSITION, &[(x / 8) as u8])
            .await?;
        Ok(())
    }

    async fn set_ram_y_address_position(&mut self, y: u16) -> Result<(), Error<E>> {
        self.write_command_data(
            command::SET_RAM_Y_ADDRESS_POSITION,
            &[(y % 256) as u8, (y / 256) as u8],
        )
        .await?;
        Ok(())
    }

    // TODO provide some parameters to control the border waveform
    async fn set_border_waveform(&mut self) -> Result<(), Error<E>> {
        self.write_command_data(command::BORDER_WAVEFORM_CONTROL, &[0b101])
            .await?;
        Ok(())
    }

    async fn set_temperature_sensor(&mut self, sensor: TemperatureSensor) -> Result<(), Error<E>> {
        self.write_command_data(command::TEMPERATURE_SENSOR_CONTROL, &[sensor as u8])
            .await?;
        Ok(())
    }

    // TODO use bitmask or something for this
    // 0xb1 before writing RAM
    // 0xc7 to display
    // 7 = enable clock signal
    // 6 = enable analog
    // 5 = Load temperature value
    // 4 = load LUT with DISPLAY Mode 1
    // 3 = load LUT or display with DISPLAY Mode 2
    // 2 = display with DISPLAY Mode 1
    // 1 = disable analog
    // 0 = disable clock signal
    async fn set_display_update_sequence(&mut self, sequence: u8) -> Result<(), Error<E>> {
        self.write_command_data(command::DISPLAY_UPDATE_CONTROL_2, &[sequence])
            .await?;
        Ok(())
    }

    async fn master_activation(&mut self) -> Result<(), Error<E>> {
        self.write_command(command::MASTER_ACTIVATION).await?;
        self.busy_wait().await;
        Ok(())
    }

    async fn write_bw_ram(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.write_command_data(command::WRITE_RAM_BW, data).await?;
        Ok(())
    }

    async fn set_deep_sleep_mode(&mut self, mode: DeepSleepMode) -> Result<(), Error<E>> {
        self.write_command_data(command::DEEP_SLEEP_MODE, &[mode as u8])
            .await?;
        Ok(())
    }

    async fn busy_wait(&mut self) {
        // self.busy.wait_for_low().await.unwrap();
        while self.busy.is_high().unwrap() {
            self.delay.delay_ms(10).await;
        }
    }

    async fn write_command_data(&mut self, command: u8, data: &[u8]) -> Result<(), Error<E>> {
        self.write_command(command).await?;
        self.write_data(data).await?;
        Ok(())
    }

    async fn write_command(&mut self, command: u8) -> Result<(), Error<E>> {
        self.dc.set_low().unwrap();
        esp_println::println!("DC low");
        self.spi.write(&[command])?;
        esp_println::println!("command written");
        Ok(())
    }

    async fn write_data(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.dc.set_high().unwrap();
        esp_println::println!("DC high");
        self.spi.write(data)?;
        esp_println::println!("data written");
        Ok(())
    }
}

impl<SPI: SpiBus, DC, RES, Busy, Delay> OriginDimensions
    for GDEH0154D67<SPI, DC, RES, Busy, Delay>
{
    fn size(&self) -> embedded_graphics_core::prelude::Size {
        embedded_graphics_core::prelude::Size {
            width: 200,
            height: 200,
        }
    }
}

impl<SPI: SpiBus<Error = E>, DC, RES, Busy, Delay, E> DrawTarget
    for GDEH0154D67<SPI, DC, RES, Busy, Delay>
{
    type Color = BinaryColor;
    type Error = Error<E>;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics_core::Pixel<Self::Color>>,
    {
        for Pixel(pos, color) in pixels.into_iter() {
            if let Ok((x @ 0..=199, y @ 0..=199)) = pos.try_into() {
                let index = (x + y * 200) as usize;
                self.buffer[index / 8] &= !(1 << (7 - (index % 8)));
                if color.is_off() {
                    self.buffer[index / 8] |= 1 << (7 - (index % 8));
                }
            }
        }

        Ok(())
    }
}
