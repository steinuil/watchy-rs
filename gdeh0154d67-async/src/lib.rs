#![no_std]

use core::convert::Infallible;

use bitflags::bitflags;
use embassy_futures::select;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::{delay::DelayNs, digital::Wait, spi::SpiBus};
use unwrap_infallible::UnwrapInfallible;

#[derive(Debug)]
pub enum Error<E> {
    Spi(E),
    BusyTimeout,
    Uninitialized,
}

impl<E: core::fmt::Display> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Spi(e) => write!(f, "Bus error: {}", e),
            Error::BusyTimeout => write!(f, "BUSY stayed high past the configured threshold"),
            Error::Uninitialized => write!(f, "panel is uninitialized"),
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
    pub const BOOSTER_SOFT_START_CONTROL: u8 = 0x0c;
    pub const DEEP_SLEEP_MODE: u8 = 0x10;
    pub const DATA_ENTRY_MODE_SETTING: u8 = 0x11;
    pub const SW_RESET: u8 = 0x12;
    pub const TEMPERATURE_SENSOR_CONTROL: u8 = 0x18;
    pub const MASTER_ACTIVATION: u8 = 0x20;
    pub const DISPLAY_UPDATE_CONTROL_1: u8 = 0x21;
    pub const DISPLAY_UPDATE_CONTROL_2: u8 = 0x22;
    pub const WRITE_RAM_BW: u8 = 0x24;
    pub const WRITE_RAM_RED: u8 = 0x26;
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

    /// AKA Deep Sleep Mode 1
    RetainRAM = 0b01,

    /// AKA Deep Sleep Mode 2
    ResetRAM = 0b11,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum BorderColor {
    White = 0b101,
    Black = 0b010,
}

/// Control how the bits in RAM are drawn to the display.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RamOptions {
    /// Set 0 bytes to black and 1 bytes to white
    Normal = 0,

    /// Ignore the RAM entirely and draw the whole window black.
    BypassAsZero = 0b100,

    /// Set 0 bytes to white and 1 bytes to black
    Invert = 0b1000,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseDuration {
    _10ms = 0b00,
    _20ms = 0b01,
    _30ms = 0b10,
    _40ms = 0b11,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseDrivingStrength {
    _1 = 0b000,
    _2 = 0b001,
    _3 = 0b010,
    _4 = 0b011,
    _5 = 0b100,
    _6 = 0b101,
    _7 = 0b110,
    _8 = 0b111,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseMinOffTime {
    _2_6 = 0b0100,
    _3_2 = 0b0101,
    _3_9 = 0b0110,
    _4_6 = 0b0111,
    _5_4 = 0b1000,
    _6_3 = 0b1001,
    _7_3 = 0b1010,
    _8_4 = 0b1011,
    _9_8 = 0b1100,
    _11_5 = 0b1101,
    _13_8 = 0b1110,
    _16_5 = 0b1111,
}

#[derive(Clone, Copy, Debug)]
pub struct BoosterPhase {
    pub driving_strength: PhaseDrivingStrength,
    pub min_off_time: PhaseMinOffTime,
    pub duration: PhaseDuration,
}

impl BoosterPhase {
    pub(crate) fn to_phase_bits(self) -> u8 {
        ((self.driving_strength as u8) << 4) | self.min_off_time as u8 | 0x80
    }
}

pub const WATCHY_BOOSTER_PHASE: BoosterPhase = BoosterPhase {
    driving_strength: PhaseDrivingStrength::_8,
    min_off_time: PhaseMinOffTime::_2_6,
    duration: PhaseDuration::_10ms,
};

#[derive(Clone, Copy, Debug)]
pub struct BoosterConfig {
    pub phase1: BoosterPhase,
    pub phase2: BoosterPhase,
    pub phase3: BoosterPhase,
}

impl BoosterConfig {
    pub(crate) fn to_bytes(self) -> [u8; 4] {
        let phase1 = self.phase1.to_phase_bits();
        let phase2 = self.phase2.to_phase_bits();
        let phase3 = self.phase3.to_phase_bits();

        let duration = self.phase1.duration as u8
            | (self.phase2.duration as u8) << 2
            | (self.phase3.duration as u8) << 4;

        [phase1, phase2, phase3, duration]
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct DataEntryMode : u8 {
        const X_INCREMENT = 1;
        const Y_INCREMENT = 1 << 1;
        const ADDR_MODE_Y = 1 << 2;

        const DEFAULT = Self::X_INCREMENT.bits() | Self::Y_INCREMENT.bits();
    }

    /// Stages of the display update sequence, run in bit order.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct DisplayUpdateSequence : u8 {
        const ENABLE_CLOCK_SIGNAL = 1 << 7;
        const ENABLE_ANALOG = 1 << 6;
        const LOAD_TEMPERATURE_VALUE = 1 << 5;

        /// Load the waveform LUT.
        ///
        /// When [`Self::USE_DISPLAY_MODE_2`], loads the mode 2 LUT instead.
        const LOAD_LUT = 1 << 4;

        /// Toggle between DISPLAY mode 1 and 2.
        const USE_DISPLAY_MODE_2 = 1 << 3;
        const DISPLAY = 1 << 2;
        const DISABLE_ANALOG = 1 << 1;
        const DISABLE_CLOCK_SIGNAL = 1;

        // 0xb1
        const LOAD_WAVEFORM_LUT_FROM_OTP = Self::ENABLE_CLOCK_SIGNAL.bits()
            | Self::LOAD_TEMPERATURE_VALUE.bits()
            | Self::LOAD_LUT.bits()
            | Self::DISABLE_CLOCK_SIGNAL.bits();

        // 0xc7
        const DRIVE_DISPLAY_PANEL = Self::ENABLE_CLOCK_SIGNAL.bits()
            | Self::ENABLE_ANALOG.bits()
            | Self::DISPLAY.bits()
            | Self::DISABLE_ANALOG.bits()
            | Self::DISABLE_CLOCK_SIGNAL.bits();

        // 0xf8
        const WATCHY_DISPLAY_POWER_ON = Self::ENABLE_CLOCK_SIGNAL.bits()
            | Self::ENABLE_ANALOG.bits()
            | Self::LOAD_TEMPERATURE_VALUE.bits()
            | Self::LOAD_LUT.bits()
            | Self::USE_DISPLAY_MODE_2.bits();

        const WATCHY_DISPLAY_POWER_OFF = Self::ENABLE_CLOCK_SIGNAL.bits()
            | Self::DISABLE_ANALOG.bits()
            | Self::DISABLE_CLOCK_SIGNAL.bits();

        // 0xfc
        const WATCHY_UPDATE_PARTIAL = Self::ENABLE_CLOCK_SIGNAL.bits()
            | Self::ENABLE_ANALOG.bits()
            | Self::LOAD_TEMPERATURE_VALUE.bits()
            | Self::LOAD_LUT.bits()
            | Self::USE_DISPLAY_MODE_2.bits()
            | Self::DISPLAY.bits();

        // 0xcc
        //
        // Partial update reusing the LUT and temperature already loaded by the
        // power-on sequence, saving ~5ms according to the Watchy firmware.
        //
        // Only valid while the controller has not been reset or sent into deep sleep
        // since that load.
        const WATCHY_UPDATE_PARTIAL_NO_RELOAD = Self::ENABLE_CLOCK_SIGNAL.bits()
            | Self::ENABLE_ANALOG.bits()
            | Self::USE_DISPLAY_MODE_2.bits()
            | Self::DISPLAY.bits();

        // 0xf4
        const WATCHY_UPDATE_FULL = Self::ENABLE_CLOCK_SIGNAL.bits()
            | Self::ENABLE_ANALOG.bits()
            | Self::LOAD_TEMPERATURE_VALUE.bits()
            | Self::LOAD_LUT.bits()
            | Self::DISPLAY.bits();
    }
}

enum PanelState {
    /// In deep sleep or not yet reset.
    Hibernating,

    /// Hardware reset done, no init block sent since.
    Reset,

    /// Init block sent.
    Initialized { partial: bool, powered: bool },
}

const WIDTH: u16 = 200;
const HEIGHT: u16 = 200;

const BUSY_SETTLE_MS: u32 = 1;
const BUSY_TIMEOUT_MS: u32 = 10_000;

pub struct GDEH0154D67<SPI, DC, RES, Busy, Delay> {
    spi: SPI,
    dc: DC,
    reset: RES,
    busy: Busy,
    delay: Delay,
    state: PanelState,
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
            state: PanelState::Hibernating,
        }
    }

    // Operation flow

    pub async fn init(&mut self) -> Result<(), Error<E>> {
        // We have to wait 10ms after power is supplied.
        self.delay.delay_ms(10).await;

        self.hardware_reset().await;
        self.software_reset().await?;

        self.set_driver_output().await?;

        Ok(())
    }

    pub async fn set_partial_ram_area(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), Error<E>> {
        self.set_data_entry_mode(DataEntryMode::DEFAULT).await?;

        self.set_ram_x_start_end_position(x, width).await?;
        self.set_ram_y_start_end_position(y, height).await?;
        self.set_ram_x_address_position(x).await?;
        self.set_ram_y_address_position(y).await?;

        Ok(())
    }

    pub async fn set_border_color(&mut self, color: BorderColor) -> Result<(), Error<E>> {
        self.set_border_waveform(color as u8).await
    }

    pub async fn load_waveform_lut_from_otp(&mut self) -> Result<(), Error<E>> {
        self.select_temperature_sensor(TemperatureSensor::Internal)
            .await?;
        self.update_display(DisplayUpdateSequence::LOAD_WAVEFORM_LUT_FROM_OTP, None)
            .await?;

        Ok(())
    }

    pub async fn write_image_data(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.write_bw_ram(data).await
    }

    /// Update the display with the contents of the RAM.
    pub async fn update_display(
        &mut self,
        sequence: DisplayUpdateSequence,
        options: Option<RamOptions>,
    ) -> Result<(), Error<E>> {
        if let Some(options) = options {
            self.set_display_update_ram_options(options).await?;
        }
        self.set_display_update_sequence(sequence).await?;
        self.master_activation().await?;

        Ok(())
    }

    pub async fn hibernate(&mut self) -> Result<(), Error<E>> {
        self.set_deep_sleep_mode(DeepSleepMode::RetainRAM).await?;
        self.state = PanelState::Hibernating;
        Ok(())
    }

    // Commands

    async fn hardware_reset(&mut self) {
        self.reset.set_low().unwrap_infallible();
        self.delay.delay_ms(10).await;
        self.reset.set_high().unwrap_infallible();
        self.delay.delay_ms(10).await;
        self.state = PanelState::Reset;
    }

    /// Resets the commands and parameters to their S/W Reset default values
    /// except Deep Sleep Mode.
    /// RAM is unaffected by this command.
    async fn software_reset(&mut self) -> Result<(), Error<E>> {
        self.write_command(command::SW_RESET).await?;
        // According to the SSD1681 spec
        self.delay.delay_ms(10).await;
        Ok(())
    }

    pub async fn booster_soft_start_control(
        &mut self,
        config: BoosterConfig,
    ) -> Result<(), Error<E>> {
        self.write_command_data(command::BOOSTER_SOFT_START_CONTROL, &config.to_bytes())
            .await
    }

    /// Write to the "previous image" plane used by DISPLAY mode 2.
    pub async fn write_previous_image_data(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.write_command_data(command::WRITE_RAM_RED, data).await
    }

    async fn set_driver_output(&mut self) -> Result<(), Error<E>> {
        // The first 9 bits set the number of vertical rows that the display has,
        // the first 3 bits of the last byte set the gate scanning sequence and direction.
        // Probably not a good idea to mess with this.
        self.write_command_data(command::DRIVER_OUTPUT_CONTROL, &[0xc7, 0x00, 0x00])
            .await?;
        Ok(())
    }

    /// Set how the X and Y coordinates are incremented while drawing to the display.
    async fn set_data_entry_mode(
        &mut self,
        data_entry_mode: DataEntryMode,
    ) -> Result<(), Error<E>> {
        self.write_command_data(command::DATA_ENTRY_MODE_SETTING, &[data_entry_mode.bits()])
            .await?;
        Ok(())
    }

    /// Set the horizontal window in display RAM where image data will be written.
    async fn set_ram_x_start_end_position(&mut self, x: u16, width: u16) -> Result<(), Error<E>> {
        self.write_command_data(
            command::SET_RAM_X_START_END_POSITION,
            &[(x / 8) as u8, ((x + width - 1) / 8) as u8],
        )
        .await?;
        Ok(())
    }

    /// Set the vertical window in display RAM where image data will be written.
    async fn set_ram_y_start_end_position(&mut self, y: u16, height: u16) -> Result<(), Error<E>> {
        self.write_command_data(
            command::SET_RAM_Y_START_END_POSITION,
            &[
                (y % 256) as u8,
                (y / 256) as u8,
                ((y + height - 1) % 256) as u8,
                ((y + height - 1) / 256) as u8,
            ],
        )
        .await?;
        Ok(())
    }

    /// Set the absolute starting X position in RAM where data will be written.
    async fn set_ram_x_address_position(&mut self, x: u16) -> Result<(), Error<E>> {
        self.write_command_data(command::SET_RAM_X_ADDRESS_POSITION, &[(x / 8) as u8])
            .await?;
        Ok(())
    }

    /// Set the absolute starting Y position in RAM where data will be written.
    async fn set_ram_y_address_position(&mut self, y: u16) -> Result<(), Error<E>> {
        self.write_command_data(
            command::SET_RAM_Y_ADDRESS_POSITION,
            &[(y % 256) as u8, (y / 256) as u8],
        )
        .await?;
        Ok(())
    }

    // TODO make an enum for border waveform or something
    // 0x02 = 0b010 = darkBorder in Display.cpp
    // 0x05 = 0b101 = normal
    async fn set_border_waveform(&mut self, border_waveform: u8) -> Result<(), Error<E>> {
        self.write_command_data(command::BORDER_WAVEFORM_CONTROL, &[border_waveform])
            .await?;
        Ok(())
    }

    pub async fn select_temperature_sensor(
        &mut self,
        sensor: TemperatureSensor,
    ) -> Result<(), Error<E>> {
        self.write_command_data(command::TEMPERATURE_SENSOR_CONTROL, &[sensor as u8])
            .await
    }

    async fn set_display_update_ram_options(
        &mut self,
        options: RamOptions,
    ) -> Result<(), Error<E>> {
        self.write_command_data(command::DISPLAY_UPDATE_CONTROL_1, &[options as u8])
            .await
    }

    async fn set_display_update_sequence(
        &mut self,
        sequence: DisplayUpdateSequence,
    ) -> Result<(), Error<E>> {
        self.write_command_data(command::DISPLAY_UPDATE_CONTROL_2, &[sequence.bits()])
            .await?;
        Ok(())
    }

    async fn master_activation(&mut self) -> Result<(), Error<E>> {
        self.write_command(command::MASTER_ACTIVATION).await?;
        self.busy_wait().await?;
        Ok(())
    }

    async fn write_bw_ram(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.write_command_data(command::WRITE_RAM_BW, data).await
    }

    async fn set_deep_sleep_mode(&mut self, mode: DeepSleepMode) -> Result<(), Error<E>> {
        self.write_command_data(command::DEEP_SLEEP_MODE, &[mode as u8])
            .await
    }

    // Helpers

    async fn busy_wait(&mut self) -> Result<(), Error<E>> {
        self.delay.delay_ms(BUSY_SETTLE_MS).await;

        let Self { busy, delay, .. } = self;

        match select::select(busy.wait_for_low(), delay.delay_ms(BUSY_TIMEOUT_MS)).await {
            select::Either::First(result) => {
                result.unwrap_infallible();
                Ok(())
            }

            select::Either::Second(()) => Err(Error::BusyTimeout),
        }
    }

    async fn write_command_data(&mut self, command: u8, data: &[u8]) -> Result<(), Error<E>> {
        self.write_command(command).await?;
        self.write_data(data).await?;
        Ok(())
    }

    async fn write_command(&mut self, command: u8) -> Result<(), Error<E>> {
        self.dc.set_low().unwrap_infallible();
        self.spi.write(&[command]).await?;
        Ok(())
    }

    async fn write_data(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.dc.set_high().unwrap_infallible();
        self.spi.write(data).await?;
        Ok(())
    }
}

// This is how Watchy's Display.cpp does things

impl<SPI, DC, RES, Busy, Delay, E> GDEH0154D67<SPI, DC, RES, Busy, Delay>
where
    SPI: SpiBus<Error = E>,
    DC: OutputPin<Error = Infallible>,
    RES: OutputPin<Error = Infallible>,
    Busy: InputPin<Error = Infallible> + Wait,
    Delay: DelayNs,
{
    // pub async fn watchy_hibernate(&mut self) -> Result<(), Error<E>> {
    //     self.set_deep_sleep_mode(DeepSleepMode::RetainRAM).await
    // }

    // _InitDisplay
    async fn watchy_init_display(&mut self, partial: bool) -> Result<(), Error<E>> {
        if matches!(self.state, PanelState::Hibernating) {
            self.hardware_reset().await;
        }
        let powered = matches!(self.state, PanelState::Initialized { powered: true, .. });

        self.software_reset().await?;

        self.set_driver_output().await?;
        self.select_temperature_sensor(TemperatureSensor::Internal)
            .await?;
        self.set_border_waveform(0b101).await?;
        self.set_partial_ram_area(0, 0, WIDTH, HEIGHT).await?;

        self.state = PanelState::Initialized { partial, powered };

        Ok(())
    }

    // _setPartialRamArea
    // async fn watchy_set_partial_ram_area(
    //     &mut self,
    //     x: u16,
    //     y: u16,
    //     width: u16,
    //     height: u16,
    // ) -> Result<(), Error<E>> {
    //     self.set_data_entry_mode(DataEntryMode::DEFAULT).await?;

    //     self.set_ram_x_start_end_position(x, width).await?;
    //     self.set_ram_y_start_end_position(y, height).await?;
    //     self.set_ram_x_address_position(x).await?;
    //     self.set_ram_y_address_position(y).await?;

    //     Ok(())
    // }

    // pub async fn watchy_write_buffer(&mut self) -> Result<(), Error<E>> {
    //     self.dc.set_low().unwrap_infallible();
    //     self.spi.write(&[command::WRITE_RAM_BW]).await?;
    //     self.dc.set_high().unwrap_infallible();
    //     self.spi.write(&self.buffer[..]).await?;

    //     Ok(())
    // }

    // _PowerOn
    async fn watchy_power_on(&mut self) -> Result<(), Error<E>> {
        match self.state {
            PanelState::Initialized { powered: true, .. } => Ok(()),
            PanelState::Initialized {
                powered: false,
                partial,
            } => {
                self.set_display_update_sequence(DisplayUpdateSequence::WATCHY_DISPLAY_POWER_ON)
                    .await?;
                self.master_activation().await?;
                self.state = PanelState::Initialized {
                    partial,
                    powered: true,
                };
                Ok(())
            }

            PanelState::Hibernating | PanelState::Reset => Err(Error::Uninitialized),
        }
    }

    // _Init_Full and _Init_Part
    async fn watchy_init(&mut self, partial: bool) -> Result<(), Error<E>> {
        if matches!(self.state, PanelState::Initialized { partial: p, powered: true } if p == partial)
        {
            return Ok(());
        }

        self.watchy_init_display(partial).await?;
        self.watchy_power_on().await?;
        Ok(())
    }

    // _PowerOff and powerOff
    pub async fn watchy_power_off(&mut self) -> Result<(), Error<E>> {
        if !matches!(self.state, PanelState::Initialized { powered: true, .. }) {
            return Ok(());
        }

        self.set_display_update_sequence(DisplayUpdateSequence::WATCHY_DISPLAY_POWER_OFF)
            .await?;
        self.master_activation().await?;
        self.state = PanelState::Reset;

        Ok(())
    }

    // _Update_Part
    async fn watchy_update_partial(&mut self) -> Result<(), Error<E>> {
        self.set_display_update_sequence(DisplayUpdateSequence::WATCHY_UPDATE_PARTIAL)
            .await?;
        self.master_activation().await?;
        Ok(())
    }

    // _Update_Full
    async fn watchy_update_full(&mut self) -> Result<(), Error<E>> {
        self.set_display_update_sequence(DisplayUpdateSequence::WATCHY_UPDATE_FULL)
            .await?;
        self.master_activation().await?;
        Ok(())
    }

    // refresh(true)
    pub async fn watchy_refresh(&mut self) -> Result<(), Error<E>> {
        self.watchy_refresh_partial(0, 0, WIDTH, HEIGHT).await
    }

    pub async fn watchy_refresh_full(&mut self) -> Result<(), Error<E>> {
        self.watchy_init(false).await?;
        self.set_partial_ram_area(0, 0, WIDTH, HEIGHT).await?;
        self.watchy_update_full().await?;
        Ok(())
    }

    // refresh(x, y, w, h)
    pub async fn watchy_refresh_partial(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), Error<E>> {
        // Clamp the update to the dimensions of the screen.
        if x >= WIDTH || y >= HEIGHT {
            return Ok(());
        }

        let width = width.min(WIDTH - x);
        let height = height.min(HEIGHT - y);
        if width == 0 || height == 0 {
            return Ok(());
        }

        let width = width + (x % 8);
        let width = if !width.is_multiple_of(8) {
            width + 8 - (width % 8)
        } else {
            width
        };
        let x = x - (x % 8);

        self.watchy_init(true).await?;
        self.set_partial_ram_area(x, y, width, height).await?;
        self.watchy_update_partial().await?;

        Ok(())
    }
}
