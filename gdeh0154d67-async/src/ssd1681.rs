use core::convert::Infallible;

use embedded_hal::digital::OutputPin;
use embedded_hal_async::{delay::DelayNs, digital::Wait, spi::SpiDevice};
use unwrap_infallible::UnwrapInfallible as _;

pub struct SSD1681<SPI, DC, RES, Busy, Delay> {
    spi: SPI,
    dc: DC,
    reset: RES,
    busy: Busy,
    delay: Delay,
    is_hibernating: bool,
    // window: Option<RamWindow>,
}

#[derive(Debug)]
pub enum Error<E> {
    Spi(E),
    BusyTimeout,
    Uninitialized,
}

impl<SPI, DC, RES, Busy, Delay, E> SSD1681<SPI, DC, RES, Busy, Delay>
where
    SPI: SpiDevice<Error = E>,
    DC: OutputPin<Error = Infallible>,
    RES: OutputPin<Error = Infallible>,
    Busy: Wait<Error = Infallible>,
    Delay: DelayNs,
{
    /// Initializes the chip after supplying power or waking from deep sleep.
    pub async fn hardware_reset(&mut self) {
        self.reset.set_low().unwrap_infallible();
        self.delay.delay_ms(10).await;
        self.reset.set_high().unwrap_infallible();
        self.delay.delay_ms(10).await;
    }

    /// Resets the commands and parameters to their S/W Reset default values,
    /// except deep sleep mode.
    ///
    /// RAM is unaffected by this command.
    pub async fn software_reset(&mut self) -> Result<(), Error<E>> {
        self.write_command(command::SW_RESET).await?;
        self.delay.delay_ms(10).await;
        Ok(())
    }

    /// Puts the controller in a low-power mode, which can only be disengaged with a
    /// [`hardware_reset`](Self::hardware_reset).
    pub async fn deep_sleep(&mut self, mode: DeepSleepMode) -> Result<(), Error<E>> {
        self.write_command_data(command::DEEP_SLEEP_MODE, &[mode as u8])
            .await
    }

    /// Sets the gate scanning sequence for the driver output.
    pub async fn set_driver_output(&mut self, cfg: DriverOutput) -> Result<(), Error<E>> {
        self.write_command_data(command::DRIVER_OUTPUT_CONTROL, &cfg.to_bytes())
            .await
    }

    pub async fn set_gate_driving_voltage(
        &mut self,
        v: GateDrivingVoltage,
    ) -> Result<(), Error<E>> {
        self.write_command_data(command::GATE_DRIVING_VOLTAGE_CONTROL, &[v as u8])
            .await
    }

    pub async fn set_source_driving_voltage(&mut self, v: SourceVoltage) -> Result<(), Error<E>> {
        self.write_command_data(
            command::SOURCE_DRIVING_VOLTAGE_CONTROL,
            &[v.vsh1 as u8, v.vsh2 as u8, v.vsl as u8],
        )
        .await
    }

    pub async fn set_booster_soft_start(&mut self, cfg: BoosterConfig) -> Result<(), Error<E>> {
        self.write_command_data(command::BOOSTER_SOFT_START_CONTROL, &cfg.to_bytes())
            .await
    }

    /// Selects whether the temperature will be read from the controller's internal
    /// sensor, or through an external sensor using I2C.
    pub async fn select_temperature_sensor(
        &mut self,
        sensor: TemperatureSensor,
    ) -> Result<(), Error<E>> {
        self.write_command_data(command::TEMPERATURE_SENSOR_CONTROL, &[sensor as u8])
            .await
    }

    pub async fn set_border_waveform(&mut self, wf: BorderWaveform) -> Result<(), Error<E>> {
        self.write_command_data(command::BORDER_WAVEFORM_CONTROL, &[wf.to_byte()])
            .await
    }

    async fn write_command_data(&mut self, command: u8, data: &[u8]) -> Result<(), Error<E>> {
        self.write_command(command).await?;
        self.write_data(data).await?;
        Ok(())
    }

    async fn write_command(&mut self, command: u8) -> Result<(), Error<E>> {
        self.dc.set_low().unwrap_infallible();
        self.spi.write(&[command]).await.map_err(Error::Spi)?;
        Ok(())
    }

    async fn write_data(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.dc.set_high().unwrap_infallible();
        self.spi.write(data).await.map_err(Error::Spi)?;
        Ok(())
    }
}

/// Whether RAM contents survive deep sleep.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSleepMode {
    /// Deep Sleep Mode 1. RAM contents are retained.
    RetainRAM = 0b01,

    /// Deep Sleep Mode 2. RAM contents are not retained.
    ResetRAM = 0b11,
}

/// First gate output channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FirstGate {
    /// Start from G0: G0, G1, G2, G3...
    #[default]
    G0 = 0,

    /// Start from G1: G1, G0, G3, G2, G5, G4...
    G1 = 1,
}

/// Gate driver scanning order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScanOrder {
    /// Sequential, left and right gates interlaced: G0, G1, G2...G199
    #[default]
    Interlaced = 0,

    /// Even gates, then odd: G0, G2, G4...G198, G1, G3..G199
    EvenThenOdd = 1,
}

/// Direction the gate driver scans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScanDirection {
    /// G0 towards G199.
    #[default]
    Forward = 0,

    /// G199 towards G0.
    Backward = 1,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid number of gate lines")]
pub struct GateLinesOutOfRange;

/// Configuration for the gate scanning sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverOutput {
    gate_lines: u16,
    first_gate: FirstGate,
    scan_order: ScanOrder,
    scan_direction: ScanDirection,
}

impl DriverOutput {
    pub const MAX_GATE_LINES: u16 = 200;

    pub const fn new(gate_lines: u16) -> Result<Self, GateLinesOutOfRange> {
        if gate_lines == 0 || gate_lines > Self::MAX_GATE_LINES {
            return Err(GateLinesOutOfRange);
        }

        Ok(Self {
            gate_lines,
            first_gate: FirstGate::G0,
            scan_order: ScanOrder::Interlaced,
            scan_direction: ScanDirection::Forward,
        })
    }

    pub const fn with_first_gate(self, first_gate: FirstGate) -> Self {
        Self { first_gate, ..self }
    }

    pub const fn with_scan_order(self, scan_order: ScanOrder) -> Self {
        Self { scan_order, ..self }
    }

    pub const fn with_scan_direction(self, scan_direction: ScanDirection) -> Self {
        Self {
            scan_direction,
            ..self
        }
    }

    pub(crate) const fn to_bytes(self) -> [u8; 3] {
        let mux = self.gate_lines - 1;
        [
            mux as u8,
            ((mux >> 8) as u8) & 0x01,
            (self.first_gate as u8) << 2 | (self.scan_order as u8) << 1 | self.scan_direction as u8,
        ]
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GateDrivingVoltage {
    _10V = 0x03,
    _10_5V = 0x04,
    _11V = 0x05,
    _11_5V = 0x06,
    _12V = 0x07,
    _12_5V = 0x08,
    _13V = 0x09,
    _13_5V = 0x0A,
    _14V = 0x0B,
    _14_5V = 0x0C,
    _15V = 0x0D,
    _15_5V = 0x0E,
    _16V = 0x0F,
    _16_5V = 0x10,
    _17V = 0x11,
    _17_5V = 0x12,
    _18V = 0x13,
    _18_5V = 0x14,
    _19V = 0x15,
    _19_5V = 0x16,
    #[default]
    _20V = 0x00,
}

/// Positive source driving voltage.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceDrivingVoltage {
    _2_4V = 0x8E,
    _2_5V = 0x8F,
    _2_6V = 0x90,
    _2_7V = 0x91,
    _2_8V = 0x92,
    _2_9V = 0x93,
    _3V = 0x94,
    _3_1V = 0x95,
    _3_2V = 0x96,
    _3_3V = 0x97,
    _3_4V = 0x98,
    _3_5V = 0x99,
    _3_6V = 0x9A,
    _3_7V = 0x9B,
    _3_8V = 0x9C,
    _3_9V = 0x9D,
    _4V = 0x9E,
    _4_1V = 0x9F,
    _4_2V = 0xA0,
    _4_3V = 0xA1,
    _4_4V = 0xA2,
    _4_5V = 0xA3,
    _4_6V = 0xA4,
    _4_7V = 0xA5,
    _4_8V = 0xA6,
    _4_9V = 0xA7,
    _5V = 0xA8,
    _5_1V = 0xA9,
    _5_2V = 0xAA,
    _5_3V = 0xAB,
    _5_4V = 0xAC,
    _5_5V = 0xAD,
    _5_6V = 0xAE,
    _5_7V = 0xAF,
    _5_8V = 0xB0,
    _5_9V = 0xB1,
    _6V = 0xB2,
    _6_1V = 0xB3,
    _6_2V = 0xB4,
    _6_3V = 0xB5,
    _6_4V = 0xB6,
    _6_5V = 0xB7,
    _6_6V = 0xB8,
    _6_7V = 0xB9,
    _6_8V = 0xBA,
    _6_9V = 0xBB,
    _7V = 0xBC,
    _7_1V = 0xBD,
    _7_2V = 0xBE,
    _7_3V = 0xBF,
    _7_4V = 0xC0,
    _7_5V = 0xC1,
    _7_6V = 0xC2,
    _7_7V = 0xC3,
    _7_8V = 0xC4,
    _7_9V = 0xC5,
    _8V = 0xC6,
    _8_1V = 0xC7,
    _8_2V = 0xC8,
    _8_3V = 0xC9,
    _8_4V = 0xCA,
    _8_5V = 0xCB,
    _8_6V = 0xCC,
    _8_7V = 0xCD,
    _8_8V = 0xCE,
    _9V = 0x23,
    _9_2V = 0x24,
    _9_4V = 0x25,
    _9_6V = 0x26,
    _9_8V = 0x27,
    _10V = 0x28,
    _10_2V = 0x29,
    _10_4V = 0x2A,
    _10_6V = 0x2B,
    _10_8V = 0x2C,
    _11V = 0x2D,
    _11_2V = 0x2E,
    _11_4V = 0x2F,
    _11_6V = 0x30,
    _11_8V = 0x31,
    _12V = 0x32,
    _12_2V = 0x33,
    _12_4V = 0x34,
    _12_6V = 0x35,
    _12_8V = 0x36,
    _13V = 0x37,
    _13_2V = 0x38,
    _13_4V = 0x39,
    _13_6V = 0x3A,
    _13_8V = 0x3B,
    _14V = 0x3C,
    _14_2V = 0x3D,
    _14_4V = 0x3E,
    _14_6V = 0x3F,
    _14_8V = 0x40,
    _15V = 0x41,
    _15_2V = 0x42,
    _15_4V = 0x43,
    _15_6V = 0x44,
    _15_8V = 0x45,
    _16V = 0x46,
    _16_2V = 0x47,
    _16_4V = 0x48,
    _16_6V = 0x49,
    _16_8V = 0x4A,
    _17V = 0x4B,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NegativeSourceVoltage {
    M5V = 0x0A,
    M5_5V = 0x0C,
    M6V = 0x0E,
    M6_5V = 0x10,
    M7V = 0x12,
    M7_5V = 0x14,
    M8V = 0x16,
    M8_5V = 0x18,
    M9V = 0x1A,
    M9_5V = 0x1C,
    M10V = 0x1E,
    M10_5V = 0x20,
    M11V = 0x22,
    M11_5V = 0x24,
    M12V = 0x26,
    M12_5V = 0x28,
    M13V = 0x2A,
    M13_5V = 0x2C,
    M14V = 0x2E,
    M14_5V = 0x30,
    #[default]
    M15V = 0x32,
    M15_5V = 0x34,
    M16V = 0x36,
    M16_5V = 0x38,
    M17V = 0x3A,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceVoltage {
    pub vsh1: SourceDrivingVoltage,
    pub vsh2: SourceDrivingVoltage,
    pub vsl: NegativeSourceVoltage,
}

impl Default for SourceVoltage {
    fn default() -> Self {
        Self {
            vsh1: SourceDrivingVoltage::_15V,
            vsh2: SourceDrivingVoltage::_5V,
            vsl: NegativeSourceVoltage::M15V,
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug)]
pub struct BoosterConfig {
    pub phase1: BoosterPhase,
    pub phase2: BoosterPhase,
    pub phase3: BoosterPhase,
}

impl Default for BoosterConfig {
    fn default() -> Self {
        Self {
            phase1: BoosterPhase {
                driving_strength: PhaseDrivingStrength::_1,
                min_off_time: PhaseMinOffTime::_8_4,
                duration: PhaseDuration::_40ms,
            },
            phase2: BoosterPhase {
                driving_strength: PhaseDrivingStrength::_2,
                min_off_time: PhaseMinOffTime::_9_8,
                duration: PhaseDuration::_40ms,
            },
            phase3: BoosterPhase {
                driving_strength: PhaseDrivingStrength::_2,
                min_off_time: PhaseMinOffTime::_3_9,
                duration: PhaseDuration::_10ms,
            },
        }
    }
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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemperatureSensor {
    External = 0x48,
    Internal = 0x80,
}

/// Which LUT drives the border in [`BorderWaveform::GsTransition`].
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderLut {
    /// Drives black.
    LUT0 = 0b00,

    /// Drives white.
    LUT1 = 0b01,

    /// On a b/w display aliases [`Self::Lut0`], on a 3-color display drives red.
    LUT2 = 0b10,

    /// On a b/w display aliases [`Self::Lut1`], on a 3-color display aliases [`Self::Lut2`].
    LUT3 = 0b11,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GsTransitionControl {
    /// Follow the LUT, but output VCOM where it would drive the red level.
    VCOMAtRed = 0,

    /// Follow the LUT.
    FollowLUT = 1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixLevel {
    VSS = 0b00,
    VSH1 = 0b01,
    VSL = 0b10,
    VSH2 = 0b11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderWaveform {
    GsTransition {
        lut: BorderLut,
        control: GsTransitionControl,
    },

    FixLevel(FixLevel),

    VCOM,

    #[default]
    HiZ,
}

impl BorderWaveform {
    pub(crate) const fn to_byte(self) -> u8 {
        match self {
            BorderWaveform::GsTransition { lut, control } => (control as u8) << 2 | lut as u8,
            BorderWaveform::FixLevel(level) => 0b01 << 6 | (level as u8) << 4,
            BorderWaveform::VCOM => 0b10 << 6,
            BorderWaveform::HiZ => 0b11 << 6,
        }
    }
}

mod command {
    pub const DRIVER_OUTPUT_CONTROL: u8 = 0x01;
    pub const GATE_DRIVING_VOLTAGE_CONTROL: u8 = 0x03;
    pub const SOURCE_DRIVING_VOLTAGE_CONTROL: u8 = 0x04;
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
