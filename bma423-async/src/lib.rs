#![no_std]

mod register;

use bitflags::bitflags;
use embedded_hal_async::{delay::DelayNs, i2c::I2c};

const CHIP_ID: u8 = 0x13;

// Probably contains firmware and configuration for the accelerometer.
const CONFIG_FILE_SIZE: usize = 0x1800;
const CONFIG_FILE: &[u8; CONFIG_FILE_SIZE] = include_bytes!("bma423_config_file.bin");

const FEATURE_SIZE: usize = 64;
const FEATURE_RW_SIZE: usize = 8;

const ASIC_INITIALIZATION_TIMEOUT_MS: u32 = 200;

const SENSOR_TIME_SYNCHRONIZATION_US: u32 = 450;

mod feature_offset {
    // This is the value of the feature config data address after it's done
    // loading the config file, which in the C driver is saved
    // and then restored every time it needs to read the features again.
    // I'm unsure as to why it starts 8 bytes before the end of the file.
    pub const START: usize = super::CONFIG_FILE_SIZE - super::FEATURE_RW_SIZE;

    pub const STEP_COUNTER_SETTINGS_26: usize = 0x36;
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct PowerMode: u8 {
        const ADVANCED_POWER_SAVE = 0b01;
        const FIFO_SELF_WAKEUP = 0b10;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SensorPower: u8 {
        const ACCELEROMETER = 0b100;
        const AUXILIARY = 0b001;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Feature: u8 {
        const STEP_DETECTOR = 0b1;
        const STEP_COUNTER  = 0b10;
        const STEP_ACTIVITY = 0b100;
        const TAP_WAKEUP    = 0b1000;
        const WRIST_TILT    = 0b10000;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SensorStatus: u8 {
        const AUXILIARY_INTERFACE_OPERATION = 0b100;
        const COMMAND_DECODER_READY         = 0b10000;
        const AUXILIARY_SENSOR_DATA_READY   = 0b100000;
        const ACCELEROMETER_DATA_READY      = 0b1000000;
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionDetection {
    AnyMotion = 0,
    NoMotion = 1,
}

// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
// pub struct MotionSettings {
//     kjkj
// }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureConfig {
    features: Feature,
    motion_detection: MotionDetection,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptPinTriggerCondition {
    Level = 0,
    Edge = 1,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptPinLevel {
    ActiveLow = 0,
    ActiveHigh = 1,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptPinDrain {
    PushPull = 0,
    OpenDrain = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InterruptPinConfig {
    pub trigger_condition: InterruptPinTriggerCondition,
    pub level: InterruptPinLevel,
    pub drain_behavior: InterruptPinDrain,
    pub output_enabled: bool,
    pub input_enabled: bool,
}

impl InterruptPinConfig {
    pub(crate) fn bits(&self) -> u8 {
        self.trigger_condition as u8
            | ((self.level as u8) << 1)
            | ((self.drain_behavior as u8) << 2)
            | (u8::from(self.output_enabled) << 3)
            | (u8::from(self.input_enabled) << 4)
    }

    pub(crate) fn from_bits_truncate(bits: u8) -> Self {
        let trigger_condition = if bits & 0b1 == 0b1 {
            InterruptPinTriggerCondition::Level
        } else {
            InterruptPinTriggerCondition::Edge
        };
        let level = if bits & 0b10 == 0b10 {
            InterruptPinLevel::ActiveLow
        } else {
            InterruptPinLevel::ActiveHigh
        };
        let drain_behavior = if bits & 0b100 == 0b100 {
            InterruptPinDrain::PushPull
        } else {
            InterruptPinDrain::OpenDrain
        };
        let output_enabled = bits & 0b1000 == 0b1000;
        let input_enabled = bits & 0b10000 == 0b10000;

        InterruptPinConfig {
            trigger_condition,
            level,
            drain_behavior,
            output_enabled,
            input_enabled,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptPin {
    Pin1 = 0,
    Pin2 = 1,
}

fn sensor_temperature_as_celsius(temp: u8) -> Option<i16> {
    if temp == 0x80 {
        None
    } else {
        Some(((temp as i8) as i16) + 23)
    }
}

// #[test]
// fn test_temperature_sensor_celsius_conversion() {
//     assert_eq!(Some(150), sensor_temperature_as_celsius(0x7f));
//     assert_eq!(Some(23), sensor_temperature_as_celsius(0x00));
//     assert_eq!(Some(-104), sensor_temperature_as_celsius(0x81));
//     assert_eq!(None, sensor_temperature_as_celsius(0x80))
// }

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InternalStatusMessage {
    NotInitialized = 0x00,
    Initialized = 0x01,
    InitializationError = 0x02,
    InvalidDriver = 0x03,
    SensorStopped = 0x04,
}

pub struct BMA423<I2C, D: DelayNs> {
    address: u8,
    i2c: I2C,
    delay: D,
}

#[derive(Debug)]
pub enum Error<E> {
    Bus(E),
    InvalidChipId(u8),
    UnknownPowerMode(u8),
    ASICInitialization,
}

impl<E: core::fmt::Display> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Error::Bus(e) => write!(f, "Bus error: {}", e),
            Error::InvalidChipId(id) => write!(f, "Invalid chip ID: {}", id),
            Error::UnknownPowerMode(id) => write!(f, "Invalid power mode: {}", id),
            Error::ASICInitialization => write!(f, "Failed to initialize ASIC"),
        }
    }
}

impl<E> core::convert::From<E> for Error<E> {
    fn from(error: E) -> Self {
        Error::Bus(error)
    }
}

impl<I2C: I2c<Error = E>, E, D: DelayNs> BMA423<I2C, D> {
    pub fn new(address: u8, i2c: I2C, delay: D) -> Self {
        BMA423 {
            address,
            i2c,
            delay,
        }
    }

    // bma4_get_error_status
    // pub async fn error_status(&mut self) -> Result<ErrorStatus, Error<E>> {}

    pub async fn sensor_status(&mut self) -> Result<SensorStatus, Error<E>> {
        let status = self.read_u8(register::STATUS).await?;
        Ok(SensorStatus::from_bits_truncate(status))
    }

    pub async fn power_mode(&mut self) -> Result<PowerMode, Error<E>> {
        let power_mode = self.read_u8(register::PWR_CONF).await?;
        Ok(PowerMode::from_bits_truncate(power_mode))
    }

    pub async fn set_power_mode(&mut self, mode: PowerMode) -> Result<(), Error<E>> {
        self.write(&[register::PWR_CONF, mode.bits()]).await
    }

    /// Get the power status of the accelerometer and auxiliary sensors.
    /// To check if the accelerometer is enabled:
    /// ```ignore
    /// let is_accelerometer_enabled = bma423
    ///     .enabled_sensors()?
    ///     .contains(SensorPower::ACCELEROMETER);
    /// ```
    pub async fn enabled_sensors(&mut self) -> Result<SensorPower, Error<E>> {
        let sensors = self.read_u8(register::PWR_CTRL).await?;
        Ok(SensorPower::from_bits_truncate(sensors))
    }

    /// Toggle the accelerometer and auxiliary sensors.
    pub async fn toggle_sensors(&mut self, sensors: SensorPower) -> Result<(), Error<E>> {
        self.write(&[register::PWR_CTRL, sensors.bits()]).await
    }

    pub async fn set_interrupt_pin_config(
        &mut self,
        pin: InterruptPin,
        config: InterruptPinConfig,
    ) -> Result<(), Error<E>> {
        self.write(&[register::INT1_IO_CTRL + pin as u8, config.bits()])
            .await
    }

    /// Temperature in Celsius in the range -104..150.
    /// Updated every 1.28s.
    /// The temperature sensor is always on when a sensor is active.
    /// When there is no valid temperature information available
    /// (i.e. last measurement before the time defined above),
    /// the temperature indicates an invalid value ([None]).
    pub async fn temperature_celsius(&mut self) -> Result<Option<i16>, Error<E>> {
        let temp = self.read_u8(register::TEMPERATURE).await?;
        Ok(sensor_temperature_as_celsius(temp))
    }

    /// Free running counter with a width of 24 bits, incrementing
    /// with a resolution of 39.0625us.
    pub async fn sensor_time(&mut self) -> Result<u32, Error<E>> {
        let mut buf: [u8; 4] = [0; 4];
        self.read_registers(register::SENSORTIME_0, &mut buf[..3])
            .await?;
        Ok(u32::from_le_bytes(buf))
    }

    // pub async fn toggle_step_features(&mut self, step: Feature) -> Result<(), Error<E>> {
    //     self.set_features(|features| {
    //         let offset = feature_offset::STEP_COUNTER_SETTINGS_26 + 1;
    //         features[offset] &= 0b111;
    //         features[offset] |= step.bits();
    //     })
    //     .await
    // }

    pub async fn step_count(&mut self) -> Result<u32, Error<E>> {
        let mut buf: [u8; 4] = [0; 4];
        self.read_registers(register::STEP_COUNTER_0, &mut buf)
            .await?;
        Ok(u32::from_le_bytes(buf))
    }

    pub async fn reset_step_counter(&mut self) -> Result<(), Error<E>> {
        self.set_features(|features| {
            // The reset mask in the C driver for the step counter is 0b100 so
            // we could just assign it to 0b100 I guess.
            features[feature_offset::STEP_COUNTER_SETTINGS_26 + 1] |= 0b100;
        })
        .await
    }

    // TODO check for status & ACCELEROMETER_DATA_READY?
    pub async fn accelerometer_xyz(&mut self) -> Result<(u16, u16, u16), Error<E>> {
        let mut buf = [0; 6];
        self.read_registers(register::DATA_8, &mut buf).await?;

        let x = ((buf[1] as u16) << 8) | buf[0] as u16;
        let y = ((buf[3] as u16) << 8) | buf[2] as u16;
        let z = ((buf[5] as u16) << 8) | buf[4] as u16;

        // In the C driver it checks if the device has a 12- or 14-bit resolution,
        // but we only support the BMA423 which has a resolution of 12 bits
        // so we don't need to do that.
        Ok((x / 0x10, y / 0x10, z / 0x10))
    }

    // TODO check for status & AUXILIARY_SENSOR_DATA_READY?
    // pub async fn auxiliary_sensor_xyzr(&mut self) -> Result<(u16, u16, u16, u16), Error<E>> {
    //     let mut buf = [0; 8];
    //     self.read_registers(register::DATA_0, &mut buf).await?;

    //     let x =
    // }

    // Initialization

    pub async fn initialize(&mut self) -> Result<(), Error<E>> {
        self.check_chip_id().await?;

        self.load_config_file().await?;

        Ok(())
    }

    /// Check that the chip ID is valid.
    async fn check_chip_id(&mut self) -> Result<(), Error<E>> {
        let chip_id = self.read_u8(register::CHIP_ID).await?;

        if chip_id == CHIP_ID {
            Ok(())
        } else {
            Err(Error::InvalidChipId(chip_id))
        }
    }

    async fn load_config_file(&mut self) -> Result<(), Error<E>> {
        // Disable advanced power save
        self.set_power_mode(PowerMode::empty()).await?;

        // Wait for sensor time synchronization
        self.delay.delay_us(SENSOR_TIME_SYNCHRONIZATION_US).await;

        // Disable config loading
        self.write(&[register::INIT_CTRL, 0x00]).await?;

        // Write the config file
        self.burst_write_features(0, CONFIG_FILE).await?;

        // Enable config loading
        self.write(&[register::INIT_CTRL, 0x01]).await?;

        // Wait for the 6kb of config file to be loaded, supposedly
        // The data sheet says it takes at most 140-150ms for the ASIC
        // to be initialized after loading the configuration file.
        // I'm not sure if this is needed or if it's best to just wait
        // for 150ms and then call it a day?
        // TODO run some tests on real hardware
        let mut total_delay_ms = 0;
        loop {
            if total_delay_ms >= ASIC_INITIALIZATION_TIMEOUT_MS {
                return Err(Error::ASICInitialization);
            }
            self.delay.delay_ms(50).await;
            total_delay_ms += 50;

            match self.read_u8(register::INTERNAL_STATUS).await? & 0xF {
                // ASIC not initialized
                0x00 => {}

                // ASIC initialized
                0x01 => break,

                // Initialization error
                0x02 => return Err(Error::ASICInitialization),

                // Other error
                _ => return Err(Error::ASICInitialization),
            }
        }

        // Re-enable advanced power save
        self.set_power_mode(PowerMode::ADVANCED_POWER_SAVE).await?;

        Ok(())
    }

    // Register r/w utilities

    async fn read_registers(&mut self, register: u8, buf: &mut [u8]) -> Result<(), Error<E>> {
        self.i2c.write_read(self.address, &[register], buf).await?;
        Ok(())
    }

    async fn read_u8(&mut self, register: u8) -> Result<u8, Error<E>> {
        let mut data: [u8; 1] = [0; 1];
        self.read_registers(register, &mut data).await?;
        Ok(data[0])
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.i2c.write(self.address, data).await?;
        Ok(())
    }

    // Feature configuration utilities

    // TODO can we just load the chunk of the feature file we're interested in
    // using the correct feature_offset instead of loading the whole 64 byte file
    // every time?
    async fn set_features<F>(&mut self, f: F) -> Result<(), Error<E>>
    where
        F: FnOnce(&mut [u8]),
    {
        // Must disable advanced power save before using the FEATURES_IN register
        let prev_power_mode = self.disable_advanced_power_save().await?;

        let mut buf = [0; FEATURE_SIZE];
        self.burst_read_features(feature_offset::START, &mut buf)
            .await?;

        f(&mut buf);

        self.burst_write_features(feature_offset::START, &buf)
            .await?;

        // Restore advanced power save if it was set before
        if prev_power_mode.contains(PowerMode::ADVANCED_POWER_SAVE) {
            self.restore_advanced_power_save(prev_power_mode).await?;
        }

        Ok(())
    }

    async fn read_features(&mut self, buf: &mut [u8]) -> Result<(), Error<E>> {
        // Must disable advanced power save before using the FEATURES_IN register
        let prev_power_mode = self.disable_advanced_power_save().await?;

        self.burst_read_features(feature_offset::START, buf).await?;

        // Restore advanced power save if it was set before
        if prev_power_mode.contains(PowerMode::ADVANCED_POWER_SAVE) {
            self.restore_advanced_power_save(prev_power_mode).await?;
        }

        Ok(())
    }

    async fn disable_advanced_power_save(&mut self) -> Result<PowerMode, Error<E>> {
        let power_mode = self.power_mode().await?;
        if power_mode.contains(PowerMode::ADVANCED_POWER_SAVE) {
            self.set_power_mode(power_mode.difference(PowerMode::ADVANCED_POWER_SAVE))
                .await?;

            self.delay.delay_us(SENSOR_TIME_SYNCHRONIZATION_US).await;
        }

        Ok(power_mode)
    }

    async fn restore_advanced_power_save(
        &mut self,
        prev_power_mode: PowerMode,
    ) -> Result<(), Error<E>> {
        self.set_power_mode(prev_power_mode).await?;

        self.delay.delay_us(SENSOR_TIME_SYNCHRONIZATION_US).await;

        Ok(())
    }

    async fn burst_write_features(
        &mut self,
        start_addr: usize,
        buf: &[u8],
    ) -> Result<(), Error<E>> {
        assert!(buf.len() % 2 == 0);

        self.set_feature_config_data_addr(start_addr).await?;

        let mut chunk: [u8; FEATURE_RW_SIZE + 1] = [0; FEATURE_RW_SIZE + 1];
        chunk[0] = register::FEATURES_IN;

        for chunk_i in 0..buf.len() / FEATURE_RW_SIZE {
            let i = chunk_i * FEATURE_RW_SIZE;

            chunk[1..].copy_from_slice(&buf[i..i + FEATURE_RW_SIZE]);
            self.write(&chunk).await?;

            self.incr_feature_config_data_addr().await?;
        }

        let overflow = buf.len() % FEATURE_RW_SIZE;
        if overflow > 0 {
            chunk[1..overflow].copy_from_slice(&buf[buf.len() - overflow..]);
            self.write(&chunk).await?;
        }

        Ok(())
    }

    async fn burst_read_features(
        &mut self,
        start_addr: usize,
        buf: &mut [u8],
    ) -> Result<(), Error<E>> {
        assert!(buf.len() % 2 == 0);

        self.set_feature_config_data_addr(start_addr).await?;

        for chunk_i in 0..buf.len() / FEATURE_RW_SIZE {
            let i = chunk_i * FEATURE_RW_SIZE;

            self.read_registers(register::FEATURES_IN, &mut buf[i..i + FEATURE_RW_SIZE])
                .await?;

            self.incr_feature_config_data_addr().await?;
        }

        let overflow = buf.len() % FEATURE_RW_SIZE;
        if overflow > 0 {
            let start = buf.len() - overflow;
            self.read_registers(register::FEATURES_IN, &mut buf[start..])
                .await?;
        }

        Ok(())
    }

    async fn feature_config_data_addr(&mut self) -> Result<usize, Error<E>> {
        let asic_lsb = self.read_u8(register::RESERVED_REG_5B).await?;
        let asic_msb = self.read_u8(register::RESERVED_REG_5C).await?;

        let addr = join_feature_conf_data_address(asic_lsb, asic_msb);
        Ok(addr)
    }

    async fn set_feature_config_data_addr(&mut self, addr: usize) -> Result<(), Error<E>> {
        let (asic_lsb, asic_msb) = split_feature_conf_data_address(addr);

        self.write(&[register::RESERVED_REG_5B, asic_lsb]).await?;
        self.write(&[register::RESERVED_REG_5C, asic_msb]).await?;

        Ok(())
    }

    async fn incr_feature_config_data_addr(&mut self) -> Result<(), Error<E>> {
        let addr = self.feature_config_data_addr().await?;
        self.set_feature_config_data_addr(addr + FEATURE_RW_SIZE)
            .await
    }
}

fn split_feature_conf_data_address(addr: usize) -> (u8, u8) {
    let asic_lsb = ((addr / 2) & 0x0F) as u8;
    let asic_msb = ((addr / 2) >> 4) as u8;
    (asic_lsb, asic_msb)
}

fn join_feature_conf_data_address(asic_lsb: u8, asic_msb: u8) -> usize {
    (((asic_msb as usize) << 4) | ((asic_lsb as usize) & 0x0F)) * 2
}

// #[test]
// fn test_split_join_feature_conf_data_address() {
//     for orig in 0..(u8::MAX as usize) {
//         let orig = orig * 2;
//         let (lsb, msb) = split_feature_conf_data_address(orig);
//         let joined = join_feature_conf_data_address(lsb, msb);
//         assert_eq!(orig, joined);
//     }
// }
