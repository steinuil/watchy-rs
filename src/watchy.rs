use bitflags::bitflags;
use bma423_async::BMA423;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::Delay;
use esp_hal::{
    gpio::{Event, Input, InputConfig, Pull, WakeConfigError, WakeupConfig},
    i2c::{self, master::I2c},
    peripherals,
    rtc_cntl::{sleep::LowPower, WakeupReason, WakeupSource},
    time::Rate,
    Async,
};
use pcf8563_async::PCF8563;
use static_cell::StaticCell;

use crate::{
    battery::Battery,
    display::{Display, DisplayConfig},
    draw_buffer::DrawBuffer,
    vibration_motor::VibrationMotor,
};

type ExternalRtc<'a> = PCF8563<I2cDevice<'a, NoopRawMutex, I2c<'static, Async>>>;

type Sensor<'a> = BMA423<I2cDevice<'a, NoopRawMutex, I2c<'static, Async>>, Delay>;

#[derive(Debug, thiserror::Error)]
pub enum Error {}

bitflags! {
    pub struct ButtonPress: u8 {
        const BOTTOM_LEFT = 1;
        const BOTTOM_RIGHT = 1 << 1;
        const TOP_LEFT = 1 << 2;
        const TOP_RIGHT = 1 << 3;
    }
}

#[derive(Debug)]
pub struct WakeupPins<'a> {
    external_rtc: Input<'a>,
    btn_bottom_left: Input<'a>,
    btn_bottom_right: Input<'a>,
    btn_top_left: Input<'a>,
    btn_top_right: Input<'a>,
}

impl<'a> WakeupPins<'a> {
    pub fn new(
        external_rtc: peripherals::GPIO27<'a>,
        btn_bottom_left: peripherals::GPIO26<'a>,
        btn_bottom_right: peripherals::GPIO4<'a>,
        btn_top_left: peripherals::GPIO25<'a>,
        btn_top_right: peripherals::GPIO35<'a>,
    ) -> Self {
        WakeupPins {
            external_rtc: Input::new(external_rtc, InputConfig::default().with_pull(Pull::Up)),
            btn_bottom_left: Input::new(
                btn_bottom_left,
                InputConfig::default().with_pull(Pull::Down),
            ),
            btn_bottom_right: Input::new(
                btn_bottom_right,
                InputConfig::default().with_pull(Pull::Down),
            ),
            btn_top_left: Input::new(btn_top_left, InputConfig::default().with_pull(Pull::Down)),
            btn_top_right: Input::new(btn_top_right, InputConfig::default().with_pull(Pull::Down)),
        }
    }

    pub fn configure_wakeup(&mut self) -> Result<(), WakeConfigError> {
        let cfg = WakeupConfig::default().with_low_power_path(true);

        self.external_rtc.listen(Event::LowLevel);
        self.external_rtc.apply_wakeup_config(&cfg)?;
        self.btn_bottom_left.listen(Event::HighLevel);
        self.btn_bottom_left.apply_wakeup_config(&cfg)?;
        self.btn_bottom_right.listen(Event::HighLevel);
        self.btn_bottom_right.apply_wakeup_config(&cfg)?;
        self.btn_top_left.listen(Event::HighLevel);
        self.btn_top_left.apply_wakeup_config(&cfg)?;
        self.btn_top_right.listen(Event::HighLevel);
        self.btn_top_right.apply_wakeup_config(&cfg)?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WakeupCause {
    /// First boot or manual reset from serial monitor
    Reset,

    /// The external RTC told us to wake up
    ExternalRtcAlarm,

    ButtonBottomLeft,
    ButtonBottomRight,
    ButtonTopLeft,
    ButtonTopRight,

    Unknown(WakeupReason),
}

pub struct Watchy<'a> {
    pub display: Display<'a>,
    pub external_rtc: ExternalRtc<'a>,
    pub sensor: Sensor<'a>,
    pub vibration_motor: VibrationMotor<'a>,
    pub battery: Battery<'a, embassy_time::Delay>,
    pub draw_buffer: DrawBuffer,
    lwpr: LowPower<'a>,
    wakeup_pins: WakeupPins<'a>,
}
impl Watchy<'_> {
    pub fn init() -> Result<Self, Error> {
        let peripherals = esp_hal::init(esp_hal::Config::default());

        let timg0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
        esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

        let i2c = I2c::new(
            peripherals.I2C0,
            i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
        )
        .expect("i2c initialization")
        .with_sda(peripherals.GPIO21)
        .with_scl(peripherals.GPIO22)
        .into_async();

        static I2C_BUS: StaticCell<Mutex<NoopRawMutex, I2c<'static, Async>>> = StaticCell::new();

        let i2c_bus = I2C_BUS.init(Mutex::new(i2c));
        let external_rtc = PCF8563::new(pcf8563_async::SLAVE_ADDRESS, I2cDevice::new(i2c_bus));
        let sensor = BMA423::new(
            bma423_async::PRIMARY_ADDRESS,
            I2cDevice::new(i2c_bus),
            Delay,
        );

        let display = Display::new(DisplayConfig {
            spi: peripherals.SPI3,
            dma: peripherals.DMA_SPI3,
            sclk: peripherals.GPIO18,
            mosi: peripherals.GPIO23,
            cs: peripherals.GPIO5,
            dc: peripherals.GPIO10,
            reset: peripherals.GPIO9,
            busy: peripherals.GPIO19,
        })
        .expect("display initialization");

        let vibration_motor = VibrationMotor::new(peripherals.GPIO13);
        let battery = Battery::new(peripherals.ADC1, peripherals.GPIO34, Delay);

        let lwpr = LowPower::new(peripherals.LPWR);

        let draw_buffer = DrawBuffer::empty();

        let wakeup_pins = WakeupPins::new(
            peripherals.GPIO27,
            peripherals.GPIO26,
            peripherals.GPIO4,
            peripherals.GPIO25,
            peripherals.GPIO35,
        );

        Ok(Watchy {
            display,
            external_rtc,
            sensor,
            vibration_motor,
            battery,
            draw_buffer,
            lwpr,
            wakeup_pins,
        })
    }

    pub fn wakeup_cause(&self) -> WakeupCause {
        let reason = esp_hal::rtc_cntl::wakeup_cause();

        if reason.is_empty() {
            WakeupCause::Reset
        } else if reason.contains(WakeupSource::Ext0) {
            WakeupCause::ExternalRtcAlarm
        } else if self.wakeup_pins.btn_bottom_left.caused_wakeup() {
            WakeupCause::ButtonBottomLeft
        } else if self.wakeup_pins.btn_bottom_right.caused_wakeup() {
            WakeupCause::ButtonBottomRight
        } else if self.wakeup_pins.btn_top_left.caused_wakeup() {
            WakeupCause::ButtonTopLeft
        } else if self.wakeup_pins.btn_top_right.caused_wakeup() {
            WakeupCause::ButtonTopRight
        } else {
            WakeupCause::Unknown(reason)
        }
    }

    pub fn hibernate(mut self) -> ! {
        self.wakeup_pins
            .configure_wakeup()
            .expect("set wakeup pins");

        self.lwpr
            .sleep_deep(esp_hal::rtc_cntl::sleep::RtcSleepConfig::deep())
    }
}
