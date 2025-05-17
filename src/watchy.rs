use core::fmt::Debug;

use defmt::Format;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use esp_hal::{
    self,
    gpio::{GpioPin, Input, Output},
    i2c::{self, master::I2c},
    peripherals::LPWR,
    reset::SleepSource,
    rtc_cntl::{
        sleep::{Ext0WakeupSource, Ext1WakeupSource},
        Rtc,
    },
    spi::{self, master::Spi},
    time::RateExtU32,
    timer::timg::TimerGroup,
    Async,
};
use static_cell::StaticCell;

use crate::{
    battery::Battery, buttons::WakeupButtons, draw_buffer::DrawBuffer,
    vibration_motor::VibrationMotor,
};

#[derive(Debug, Format)]
pub enum Error {
    I2cConfig(i2c::master::ConfigError),
    SpiConfig(spi::master::ConfigError),
    Spi(spi::Error),
    Interrupt(esp_hal::interrupt::Error),
}

impl From<i2c::master::ConfigError> for Error {
    fn from(value: i2c::master::ConfigError) -> Self {
        Error::I2cConfig(value)
    }
}

impl From<spi::master::ConfigError> for Error {
    fn from(value: spi::master::ConfigError) -> Self {
        Error::SpiConfig(value)
    }
}

impl From<esp_hal::interrupt::Error> for Error {
    fn from(value: esp_hal::interrupt::Error) -> Self {
        Error::Interrupt(value)
    }
}

impl From<gdeh0154d67_async::Error<spi::Error>> for Error {
    fn from(value: gdeh0154d67_async::Error<spi::Error>) -> Self {
        match value {
            gdeh0154d67_async::Error::Spi(spi) => Error::Spi(spi),
        }
    }
}

type I2cBusDevice<'a> = I2cDevice<'a, NoopRawMutex, I2c<'static, Async>>;

type Display<'a> = gdeh0154d67_async::GDEH0154D67<
    Spi<'a, esp_hal::Async>,
    Output<'a>,
    Output<'a>,
    Input<'a>,
    embassy_time::Delay,
>;

/// GPIO pins used to wake up the device during sleep
pub struct WakeupPins {
    external_rtc: GpioPin<27>,
    btn_bottom_left: GpioPin<26>,
    btn_bottom_right: GpioPin<4>,
    btn_top_left: GpioPin<25>,
    btn_top_right: GpioPin<35>,
}

pub enum WakeupCause {
    /// First boot or manual reset from serial monitor
    Reset,

    /// The external RTC told us to wake up
    ExternalRtcAlarm,

    /// One (or more?) of the buttons was pressed
    ButtonPress(WakeupButtons),

    /// Probably shouldn't happen
    // TODO turn into Error?
    Unknown(SleepSource),
}

pub struct Watchy<'a> {
    pub display: Display<'a>,
    pub external_rtc: pcf8563_async::PCF8563<I2cBusDevice<'a>>,
    pub sensor: bma423_async::BMA423<I2cBusDevice<'a>, embassy_time::Delay>,
    pub vibration_motor: VibrationMotor<'a>,
    pub battery: Battery<'a, embassy_time::Delay>,
    pub draw_buffer: DrawBuffer,
    lpwr: LPWR,
    wakeup_pins: WakeupPins,
}

impl Watchy<'_> {
    pub fn init() -> Result<Self, Error> {
        let config = esp_hal::Config::default();
        let peripherals = esp_hal::init(config);

        // Initialize embassy
        let timer_group0 = TimerGroup::new(peripherals.TIMG0);
        esp_hal_embassy::init(timer_group0.timer0);

        defmt::debug!("initialized embassy");

        // TODO is this necessary?
        // Enable i2c for communication with PCF8563 and BMA423
        esp_hal::interrupt::enable(
            esp_hal::peripherals::Interrupt::I2C_EXT0,
            esp_hal::interrupt::Priority::Priority1,
        )?;
        // Enable SPI for communication with GDEH0154D67
        esp_hal::interrupt::enable(
            esp_hal::peripherals::Interrupt::SPI3,
            esp_hal::interrupt::Priority::Priority1,
        )?;
        defmt::debug!("enabled interrupts");

        // Initialize I2C bus
        let i2c_config = i2c::master::Config::default().with_frequency(400_u32.kHz());
        let i2c = I2c::new(peripherals.I2C0, i2c_config)?
            .with_sda(peripherals.GPIO21)
            .with_scl(peripherals.GPIO22)
            .into_async();
        static I2C_BUS: StaticCell<Mutex<NoopRawMutex, I2c<'static, Async>>> = StaticCell::new();
        let i2c_bus = I2C_BUS.init(Mutex::new(i2c));
        defmt::debug!("initialized I2C bus");

        // Initialize SPI
        // Lowered from 20MHz because it got stuck on writing data.
        let spi_config = spi::master::Config::default()
            .with_frequency(16_u32.MHz())
            .with_mode(spi::Mode::_0);
        let spi = Spi::new(peripherals.SPI3, spi_config)?
            .with_sck(peripherals.GPIO18)
            .with_mosi(peripherals.GPIO23)
            .with_cs(peripherals.GPIO5)
            .into_async();
        defmt::debug!("initialized SPI");

        // Initialize display
        // TODO check if the pin initial values are correct
        let gdeh0154d67 = gdeh0154d67_async::GDEH0154D67::new(
            spi,
            Output::new(peripherals.GPIO10, esp_hal::gpio::Level::Low),
            Output::new(peripherals.GPIO9, esp_hal::gpio::Level::Low),
            Input::new(peripherals.GPIO19, esp_hal::gpio::Pull::Up),
            embassy_time::Delay,
        );
        defmt::debug!("initialized display");

        // Initialize RTC
        let i2c_device = I2cDevice::new(i2c_bus);
        let pcf8563 = pcf8563_async::PCF8563::new(pcf8563_async::SLAVE_ADDRESS, i2c_device);
        defmt::debug!("initialized external rtc");

        // Initialize sensor
        let i2c_device = I2cDevice::new(i2c_bus);
        let bma423 = bma423_async::BMA423::new(
            bma423_async::PRIMARY_ADDRESS,
            i2c_device,
            embassy_time::Delay,
        );
        defmt::debug!("initialized sensor");

        // Initialize vibration motor
        let vibration_motor = VibrationMotor::new(peripherals.GPIO13);

        // Initialize wakeup pins
        let wakeup_pins = WakeupPins {
            external_rtc: peripherals.GPIO27,
            btn_bottom_left: peripherals.GPIO26,
            btn_bottom_right: peripherals.GPIO4,
            btn_top_left: peripherals.GPIO25,
            btn_top_right: peripherals.GPIO35,
        };

        let lpwr: LPWR = peripherals.LPWR;

        let battery = Battery::new(peripherals.ADC1, peripherals.GPIO34, embassy_time::Delay);

        let draw_buffer = DrawBuffer::empty();

        Ok(Watchy {
            display: gdeh0154d67,
            external_rtc: pcf8563,
            sensor: bma423,
            vibration_motor,
            battery,
            draw_buffer,
            lpwr,
            wakeup_pins,
        })
    }

    pub fn get_wakeup_cause(&self) -> WakeupCause {
        match esp_hal::reset::wakeup_cause() {
            SleepSource::Undefined => WakeupCause::Reset,
            SleepSource::Ext0 => WakeupCause::ExternalRtcAlarm,
            SleepSource::Ext1 => {
                let buttons = WakeupButtons::from_wakeup_status(&self.lpwr);
                WakeupCause::ButtonPress(buttons)
            }
            cause => WakeupCause::Unknown(cause),
        }
    }

    pub fn sleep_deep(&mut self) -> ! {
        let mut rtc = Rtc::new(&mut self.lpwr);

        rtc.sleep_deep(&[
            &Ext0WakeupSource::new(
                &mut self.wakeup_pins.external_rtc,
                esp_hal::rtc_cntl::sleep::WakeupLevel::Low,
            ),
            &Ext1WakeupSource::new(
                &mut [
                    &mut self.wakeup_pins.btn_bottom_left,
                    &mut self.wakeup_pins.btn_bottom_right,
                    &mut self.wakeup_pins.btn_top_left,
                    &mut self.wakeup_pins.btn_top_right,
                ],
                esp_hal::rtc_cntl::sleep::WakeupLevel::High,
            ),
        ])
    }

    pub async fn draw_buffer_to_display(&mut self) -> Result<(), Error> {
        self.display.init().await?;
        self.display
            .set_border_color(gdeh0154d67_async::BorderColor::White)
            .await?;

        self.display.set_partial_ram_area(0, 0, 200, 200).await?;
        self.display
            .write_image_data(self.draw_buffer.buffer())
            .await?;
        self.display
            .select_temperature_sensor(gdeh0154d67_async::TemperatureSensor::Internal)
            .await?;
        self.display
            .update_display(
                gdeh0154d67_async::DisplayUpdateSequence::WATCHY_UPDATE_FULL
                    | gdeh0154d67_async::DisplayUpdateSequence::DISABLE_ANALOG
                    | gdeh0154d67_async::DisplayUpdateSequence::DISABLE_CLOCK_SIGNAL,
                None,
            )
            .await?;

        self.display.hibernate().await?;

        Ok(())
    }
}
