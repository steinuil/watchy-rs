use core::num::NonZeroU8;

use embassy_time::{Duration, Timer};
use embedded_hal_async::spi::SpiDevice;
use esp32_hal::{
    clock::{ClockControl, Clocks},
    embassy,
    gpio::{GpioPin, Input, Output, OutputPin, PullUp, PushPull, Unknown},
    i2c::I2C,
    interrupt,
    pdma::{Dma, Spi3DmaChannel},
    peripherals::{Interrupt, Peripherals, I2C0, LPWR, SPI3},
    prelude::_fugit_RateExtU32,
    reset::SleepSource,
    rtc_cntl::sleep::{Ext0WakeupSource, Ext1WakeupSource, WakeupLevel},
    spi::{
        master::prelude::*,
        master::{dma::SpiDma, Spi},
        FullDuplexMode, SpiMode,
    },
    system::SystemExt,
    timer::TimerGroup,
    Delay, Rtc, IO,
};

const RTCIO_GPIO4_CHANNEL: u32 = 1 << 10;
const RTCIO_GPIO25_CHANNEL: u32 = 1 << 6;
const RTCIO_GPIO26_CHANNEL: u32 = 1 << 7;
const RTCIO_GPIO35_CHANNEL: u32 = 1 << 5;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Button {
    BottomLeft,
    TopLeft,
    TopRight,
    BottomRight,
}

fn get_ext1_wakeup_button(rtc_cntl: &LPWR) -> Result<Button, u32> {
    // TODO when esp32_hal lets you read the wakeup status, it'd be nice to use that
    // instead of using unsafe.
    let wakeup_bits = rtc_cntl.ext_wakeup1_status().read().bits();

    match wakeup_bits {
        RTCIO_GPIO26_CHANNEL => Ok(Button::BottomLeft),
        RTCIO_GPIO25_CHANNEL => Ok(Button::TopLeft),
        RTCIO_GPIO35_CHANNEL => Ok(Button::TopRight),
        RTCIO_GPIO4_CHANNEL => Ok(Button::BottomRight),
        _ => Err(wakeup_bits),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum WakeupCause {
    /// First boot or manual reset from serial monitor
    Reset,
    /// The PCF8563 RTC told us to wake up
    ExternalRtcAlarm,
    /// One of the buttons was pressed
    ButtonPress(Button),
    // Probably shouldn't happen since we only set those pins for waking up
    // TODO turn into Error
    UnknownExt1(u32),
    // Probably shouldn't happen
    // TODO turn into Error
    Unknown(SleepSource),
}

pub fn get_wakeup_cause(rtc_cntl: &LPWR) -> WakeupCause {
    let cause = esp32_hal::reset::get_wakeup_cause();

    match cause {
        SleepSource::Ext0 => WakeupCause::ExternalRtcAlarm,
        SleepSource::Ext1 => match get_ext1_wakeup_button(rtc_cntl) {
            Ok(button) => WakeupCause::ButtonPress(button),
            Err(mask) => WakeupCause::UnknownExt1(mask),
        },
        SleepSource::Undefined => WakeupCause::Reset,
        _ => WakeupCause::Unknown(cause),
    }
}

pub mod pins {
    use esp32_hal::gpio::{GpioPin, Input, Output, PullUp, PushPull, Unknown};

    // I2C
    pub const SDA: u8 = 21;
    pub type Sda = GpioPin<Unknown, SDA>;
    pub const SCL: u8 = 22;
    pub type Scl = GpioPin<Unknown, SCL>;

    // SPI
    pub const SCK: u8 = 18;
    pub type Sck = GpioPin<Unknown, SCK>;
    pub const MOSI: u8 = 23;
    pub type Mosi = GpioPin<Unknown, MOSI>;
    pub const CS: u8 = 5;
    pub type Cs = GpioPin<Output<PushPull>, CS>;

    // Display
    pub const DC: u8 = 10;
    pub type Dc = GpioPin<Output<PushPull>, DC>;
    pub const RESET: u8 = 9;
    pub type Reset = GpioPin<Output<PushPull>, RESET>;
    pub const BUSY: u8 = 19;
    pub type Busy = GpioPin<Input<PullUp>, BUSY>;

    // External RTC interrupt
    pub const EXTERNAL_RTC: u8 = 27;
    pub type ExternalRtc = GpioPin<Unknown, EXTERNAL_RTC>;

    // Button interrupts
    pub const BUTTON_BOTTOM_LEFT: u8 = 26;
    pub type ButtonBottomLeft = GpioPin<Unknown, BUTTON_BOTTOM_LEFT>;
    pub const BUTTON_BOTTOM_RIGHT: u8 = 4;
    pub type ButtonBottomRight = GpioPin<Unknown, BUTTON_BOTTOM_RIGHT>;
    pub const BUTTON_TOP_LEFT: u8 = 25;
    pub type ButtonTopLeft = GpioPin<Unknown, BUTTON_TOP_LEFT>;
    pub const BUTTON_TOP_RIGHT: u8 = 35;
    pub type ButtonTopRight = GpioPin<Unknown, BUTTON_TOP_RIGHT>;

    pub const VIBRATION_MOTOR: u8 = 13;
    pub type VibrationMotor = GpioPin<Output<PushPull>, VIBRATION_MOTOR>;

    pub struct ButtonPins {
        pub bottom_left: ButtonBottomLeft,
        pub bottom_right: ButtonBottomRight,
        pub top_left: ButtonTopLeft,
        pub top_right: ButtonTopRight,
    }

    pub struct DisplayPins {
        pub dc: Dc,
        pub reset: Reset,
        pub busy: Busy,
    }

    pub struct SpiPins {
        pub sck: Sck,
        pub mosi: Mosi,
        pub cs: Cs,
    }

    pub struct I2cPins {
        pub sda: Sda,
        pub scl: Scl,
    }

    pub struct Pins {
        pub vibration_motor: VibrationMotor,
        pub buttons: ButtonPins,
        pub external_rtc: ExternalRtc,
        pub display: DisplayPins,
        pub spi: SpiPins,
        pub i2c: I2cPins,
    }

    impl Pins {
        pub fn new(pins: esp32_hal::gpio::Pins) -> Self {
            Pins {
                vibration_motor: pins.gpio13.into_push_pull_output(),
                buttons: ButtonPins {
                    bottom_left: pins.gpio26,
                    bottom_right: pins.gpio4,
                    top_left: pins.gpio25,
                    top_right: pins.gpio35,
                },
                external_rtc: pins.gpio27,
                display: DisplayPins {
                    dc: pins.gpio10.into_push_pull_output(),
                    reset: pins.gpio9.into_push_pull_output(),
                    busy: pins.gpio19.into_pull_up_input(),
                },
                spi: SpiPins {
                    sck: pins.gpio18,
                    mosi: pins.gpio23,
                    cs: pins.gpio5.into_push_pull_output(),
                },
                i2c: I2cPins {
                    sda: pins.gpio21,
                    scl: pins.gpio22,
                },
            }
        }
    }
}

pub use pins::Pins;

pub struct VibrationMotor {
    pin: pins::VibrationMotor,
}

impl VibrationMotor {
    pub fn new(pin: pins::VibrationMotor) -> Self {
        VibrationMotor { pin }
    }

    pub fn enable(&mut self) {
        self.pin.set_output_high(true);
    }

    pub fn disable(&mut self) {
        self.pin.set_output_high(false);
    }

    pub async fn vibrate_linear(&mut self, times: u8, interval: Duration) {
        for _ in 0..times - 1 {
            self.enable();
            Timer::after(interval).await;
            self.disable();
            Timer::after(interval).await;
        }

        // Let's not wait after the last vibration
        self.enable();
        Timer::after(interval).await;
        self.disable();
    }
}

pub fn sleep_deep(
    mut rtc: Rtc,
    mut delay: Delay,
    mut interrupt_pin: pins::ExternalRtc,
    mut button_pins: pins::ButtonPins,
) -> ! {
    rtc.sleep_deep(
        &[
            &Ext0WakeupSource::new(&mut interrupt_pin, WakeupLevel::Low),
            &Ext1WakeupSource::new(
                &mut [
                    &mut button_pins.bottom_left,
                    &mut button_pins.bottom_right,
                    &mut button_pins.top_left,
                    &mut button_pins.top_right,
                ],
                WakeupLevel::High,
            ),
        ],
        &mut delay,
    );
}

// TODO: use an embassy_sync::mutex::Mutex to share the i2c bus between the devices
pub fn init_i2c<'d>(i2c0: I2C0, pins: pins::I2cPins, clocks: &Clocks) -> I2C<'d, I2C0> {
    I2C::new(i2c0, pins.sda, pins.scl, 400_u32.kHz(), clocks)
}

type Display<'a> = gdeh0154d67_async::GDEH0154D67<
    SpiDma<'a, SPI3, Spi3DmaChannel, FullDuplexMode>,
    GpioPin<Output<PushPull>, 10>,
    GpioPin<Output<PushPull>, 9>,
    GpioPin<Input<PullUp>, 19>,
    embassy_time::Delay,
>;

// struct DisplayDriver<'a, 'd> {
// pub spi_bus_controller: SpiBusController<'a, SPI3, FullDuplexMode>,
// pub display: Display<'a, 'd>,
// }

struct ExtPins {
    pub rtc_interrupt: GpioPin<Unknown, 27>,
    pub button_bottom_left: GpioPin<Unknown, 26>,
    pub button_top_left: GpioPin<Unknown, 25>,
    pub button_top_right: GpioPin<Unknown, 35>,
    pub button_bottom_right: GpioPin<Unknown, 4>,
}

pub struct Watchy<'a> {
    clocks: Clocks<'a>,
    rtc: Rtc<'a>,
    i2c: I2C<'a, I2C0>,
    // spi_bus_controller: SpiBusController<'a, SPI3, FullDuplexMode>,
    // display: Display<'a>,
    // display_driver: DisplayDriver<'a>,
    ext_pins: ExtPins,
    vibration_motor: VibrationMotor,
}

impl<'a> Watchy<'a> {
    pub async fn new() -> Result<Self, interrupt::Error> {
        let peripherals = Peripherals::take();
        let system = peripherals.SYSTEM.split();
        let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

        let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);

        // Initialize embassy
        embassy::init(&clocks, timer_group0.timer0);

        // Enable i2c for communication with PCF8563 and BMA423
        esp32_hal::interrupt::enable(
            Interrupt::I2C_EXT0,
            esp32_hal::interrupt::Priority::Priority1,
        )?;

        // Enable SPI for communication with GDEH0154D67
        esp32_hal::interrupt::enable(Interrupt::SPI3, esp32_hal::interrupt::Priority::Priority1)?;

        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

        // Pins for i2c
        let sda_pin = io.pins.gpio21;
        let scl_pin = io.pins.gpio22;

        // Pins for SPI
        let sck_pin = io.pins.gpio18;
        let mosi_pin = io.pins.gpio23;
        let cs_pin = io.pins.gpio5;

        // Pins for driving the display
        let dc_pin = io.pins.gpio10;
        let reset_pin = io.pins.gpio9;
        let busy_pin = io.pins.gpio19;

        let vibration_motor_pin = io.pins.gpio13;

        let rtc_interrupt_pin = io.pins.gpio27;

        let button_bottom_left_pin = io.pins.gpio26;
        let button_top_left_pin = io.pins.gpio25;
        let button_top_right_pin = io.pins.gpio35;
        let button_bottom_right_pin = io.pins.gpio4;

        // TODO: maybe use an embassy_sync::mutex::Mutex to share the i2c bus
        // between the devices
        let i2c = I2C::new(peripherals.I2C0, sda_pin, scl_pin, 400_u32.kHz(), &clocks);

        let spi = Spi::new(peripherals.SPI3, 20_u32.MHz(), SpiMode::Mode0, &clocks)
            .with_sck(sck_pin)
            .with_mosi(mosi_pin);

        let _spi = embedded_hal_bus::spi::ExclusiveDevice::new(
            spi,
            cs_pin.into_push_pull_output(),
            embassy_time::Delay,
        );

        // embedded_hal_async::spi::SpiDevice::write(&mut spi, &[0]).await;

        // let display = gdeh0154d67_async::GDEH0154D67::new(
        //     spi,
        //     dc_pin.into_push_pull_output(),
        //     reset_pin.into_push_pull_output(),
        //     busy_pin.into_pull_up_input(),
        //     embassy_time::Delay,
        // );

        let vibration_motor = VibrationMotor::new(vibration_motor_pin.into_push_pull_output());
        // let mut pcf8563 = pcf8563_async::PCF8563::new(pcf8563_async::SLAVE_ADDRESS, &mut i2c);

        let _status = peripherals.LPWR.ext_wakeup1_status();

        let rtc = Rtc::new(peripherals.LPWR);

        Ok(Watchy {
            clocks,
            rtc,
            i2c,
            // spi_bus_controller,
            // display,
            ext_pins: ExtPins {
                rtc_interrupt: rtc_interrupt_pin,
                button_bottom_left: button_bottom_left_pin,
                button_top_left: button_top_left_pin,
                button_top_right: button_top_right_pin,
                button_bottom_right: button_bottom_right_pin,
            },
            // display,
            vibration_motor,
        })
    }

    pub fn vibration_motor(&mut self) -> &mut VibrationMotor {
        &mut self.vibration_motor
    }

    pub fn deep_sleep(mut self) -> ! {
        let mut delay = Delay::new(&self.clocks);

        self.rtc.sleep_deep(
            &[
                &Ext0WakeupSource::new(&mut self.ext_pins.rtc_interrupt, WakeupLevel::Low),
                &Ext1WakeupSource::new(
                    &mut [
                        &mut self.ext_pins.button_bottom_left,
                        &mut self.ext_pins.button_top_left,
                        &mut self.ext_pins.button_top_right,
                        &mut self.ext_pins.button_bottom_right,
                    ],
                    WakeupLevel::High,
                ),
            ],
            &mut delay,
        )
    }
}
