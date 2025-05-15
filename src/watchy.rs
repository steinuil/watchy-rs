use embassy_time::{Duration, Timer};
use esp_hal::{
    gpio::{GpioPin, Output},
    i2c::master::{Config, I2c},
    peripherals::{I2C0, LPWR},
    reset::SleepSource,
    rtc_cntl::{
        sleep::{Ext0WakeupSource, Ext1WakeupSource, TimerWakeupSource, WakeupLevel},
        Rtc,
    },
    time::RateExtU32,
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
    let cause = esp_hal::reset::wakeup_cause();

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
    use esp_hal::{gpio::GpioPin, peripherals::Peripherals};

    // I2C
    pub const SDA: u8 = 21;
    pub type Sda = GpioPin<SDA>;
    pub const SCL: u8 = 22;
    pub type Scl = GpioPin<SCL>;

    // SPI
    pub const SCK: u8 = 18;
    pub type Sck = GpioPin<SCK>;
    pub const MOSI: u8 = 23;
    pub type Mosi = GpioPin<MOSI>;
    pub const CS: u8 = 5;
    pub type Cs = GpioPin<CS>;

    // Display
    pub const DC: u8 = 10;
    pub type Dc = GpioPin<DC>;
    pub const RESET: u8 = 9;
    pub type Reset = GpioPin<RESET>;
    pub const BUSY: u8 = 19;
    pub type Busy = GpioPin<BUSY>;

    // External RTC interrupt
    pub const EXTERNAL_RTC: u8 = 27;
    pub type ExternalRtc = GpioPin<EXTERNAL_RTC>;

    // Button interrupts
    pub const BUTTON_BOTTOM_LEFT: u8 = 26;
    pub type ButtonBottomLeft = GpioPin<BUTTON_BOTTOM_LEFT>;
    pub const BUTTON_BOTTOM_RIGHT: u8 = 4;
    pub type ButtonBottomRight = GpioPin<BUTTON_BOTTOM_RIGHT>;
    pub const BUTTON_TOP_LEFT: u8 = 25;
    pub type ButtonTopLeft = GpioPin<BUTTON_TOP_LEFT>;
    pub const BUTTON_TOP_RIGHT: u8 = 35;
    pub type ButtonTopRight = GpioPin<BUTTON_TOP_RIGHT>;

    pub const VIBRATION_MOTOR: u8 = 13;
    pub type VibrationMotor = GpioPin<VIBRATION_MOTOR>;

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
        pub fn new(peripherals: Peripherals) -> Self {
            Pins {
                vibration_motor: peripherals.GPIO13,
                buttons: ButtonPins {
                    bottom_left: peripherals.GPIO26,
                    bottom_right: peripherals.GPIO4,
                    top_left: peripherals.GPIO25,
                    top_right: peripherals.GPIO35,
                },
                external_rtc: peripherals.GPIO27,
                display: DisplayPins {
                    dc: peripherals.GPIO10,
                    reset: peripherals.GPIO9,
                    busy: peripherals.GPIO19,
                },
                spi: SpiPins {
                    sck: peripherals.GPIO18,
                    mosi: peripherals.GPIO23,
                    cs: peripherals.GPIO5,
                },
                i2c: I2cPins {
                    sda: peripherals.GPIO21,
                    scl: peripherals.GPIO22,
                },
            }
        }
    }
}

pub struct VibrationMotor<'a> {
    pin: Output<'a>,
}

impl VibrationMotor<'_> {
    pub fn new(pin: pins::VibrationMotor) -> Self {
        VibrationMotor {
            pin: Output::new(pin, esp_hal::gpio::Level::Low),
        }
    }

    pub fn enable(&mut self) {
        self.pin.set_high();
    }

    pub fn disable(&mut self) {
        self.pin.set_low();
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
    duration: core::time::Duration,
    mut interrupt_pin: pins::ExternalRtc,
    mut button_pins: pins::ButtonPins,
) -> ! {
    rtc.sleep_deep(&[
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
        &TimerWakeupSource::new(duration),
    ]);
}

// TODO: use an embassy_sync::mutex::Mutex to share the i2c bus between the devices
pub fn init_i2c<'d>(i2c0: I2C0, pins: pins::I2cPins) -> I2c<'d, esp_hal::Async> {
    I2c::new(i2c0, Config::default().with_frequency(400_u32.kHz()))
        .unwrap()
        .with_sda(pins.sda)
        .with_scl(pins.scl)
        .into_async()
}

// type Display<'a> = gdeh0154d67_async::GDEH0154D67<
//     SpiDma<'a, SPI3, Spi3DmaChannel, FullDuplexMode>,
//     GpioPin<Output<PushPull>, 10>,
//     GpioPin<Output<PushPull>, 9>,
//     GpioPin<Input<PullUp>, 19>,
//     embassy_time::Delay,
// >;

// // struct DisplayDriver<'a, 'd> {
// // pub spi_bus_controller: SpiBusController<'a, SPI3, FullDuplexMode>,
// // pub display: Display<'a, 'd>,
// // }

struct ExtPins {
    pub rtc_interrupt: GpioPin<27>,
    pub button_bottom_left: GpioPin<26>,
    pub button_top_left: GpioPin<25>,
    pub button_top_right: GpioPin<35>,
    pub button_bottom_right: GpioPin<4>,
}

// pub struct Watchy<'a> {
//     clocks: Clocks<'a>,
//     rtc: Rtc<'a>,
//     i2c: I2C<'a, I2C0>,
//     // spi_bus_controller: SpiBusController<'a, SPI3, FullDuplexMode>,
//     // display: Display<'a>,
//     // display_driver: DisplayDriver<'a>,
//     ext_pins: ExtPins,
//     vibration_motor: VibrationMotor<'a>,
// }

// impl<'a> Watchy<'a> {
//     pub async fn new() -> Result<Self, interrupt::Error> {
//         let peripherals = Peripherals::take();
//         let system = peripherals.SYSTEM.split();
//         let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

//         let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);

//         // Initialize embassy
//         embassy::init(&clocks, timer_group0);

//         // Enable i2c for communication with PCF8563 and BMA423
//         esp32_hal::interrupt::enable(
//             Interrupt::I2C_EXT0,
//             esp32_hal::interrupt::Priority::Priority1,
//         )?;

//         // Enable SPI for communication with GDEH0154D67
//         esp32_hal::interrupt::enable(Interrupt::SPI3, esp32_hal::interrupt::Priority::Priority1)?;

//         let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

//         // Pins for i2c
//         let sda_pin = io.pins.gpio21;
//         let scl_pin = io.pins.gpio22;

//         // Pins for SPI
//         let sck_pin = io.pins.gpio18;
//         let mosi_pin = io.pins.gpio23;
//         let cs_pin = io.pins.gpio5;

//         // Pins for driving the display
//         let dc_pin = io.pins.gpio10;
//         let reset_pin = io.pins.gpio9;
//         let busy_pin = io.pins.gpio19;

//         let vibration_motor_pin = io.pins.gpio13;

//         let rtc_interrupt_pin = io.pins.gpio27;

//         let button_bottom_left_pin = io.pins.gpio26;
//         let button_top_left_pin = io.pins.gpio25;
//         let button_top_right_pin = io.pins.gpio35;
//         let button_bottom_right_pin = io.pins.gpio4;

//         // TODO: maybe use an embassy_sync::mutex::Mutex to share the i2c bus
//         // between the devices
//         let i2c = I2C::new(peripherals.I2C0, sda_pin, scl_pin, 400_u32.kHz(), &clocks);

//         let spi = Spi::new(peripherals.SPI3, 20_u32.MHz(), SpiMode::Mode0, &clocks)
//             .with_sck(sck_pin)
//             .with_mosi(mosi_pin);

//         let _spi = embedded_hal_bus::spi::ExclusiveDevice::new(
//             spi,
//             cs_pin.into_push_pull_output(),
//             embassy_time::Delay,
//         );

//         // embedded_hal_async::spi::SpiDevice::write(&mut spi, &[0]).await;

//         // let display = gdeh0154d67_async::GDEH0154D67::new(
//         //     spi,
//         //     dc_pin.into_push_pull_output(),
//         //     reset_pin.into_push_pull_output(),
//         //     busy_pin.into_pull_up_input(),
//         //     embassy_time::Delay,
//         // );

//         let vibration_motor = VibrationMotor::new(vibration_motor_pin.into_push_pull_output());
//         // let mut pcf8563 = pcf8563_async::PCF8563::new(pcf8563_async::SLAVE_ADDRESS, &mut i2c);

//         let _status = peripherals.LPWR.ext_wakeup1_status();

//         let rtc = Rtc::new(peripherals.LPWR);

//         Ok(Watchy {
//             clocks,
//             rtc,
//             i2c,
//             // spi_bus_controller,
//             // display,
//             ext_pins: ExtPins {
//                 rtc_interrupt: rtc_interrupt_pin,
//                 button_bottom_left: button_bottom_left_pin,
//                 button_top_left: button_top_left_pin,
//                 button_top_right: button_top_right_pin,
//                 button_bottom_right: button_bottom_right_pin,
//             },
//             // display,
//             vibration_motor,
//         })
//     }

//     pub fn vibration_motor(&mut self) -> &mut VibrationMotor {
//         &mut self.vibration_motor
//     }

//     pub fn deep_sleep(mut self) -> ! {
//         let mut delay = Delay::new(&self.clocks);

//         self.rtc.sleep_deep(
//             &[
//                 &Ext0WakeupSource::new(&mut self.ext_pins.rtc_interrupt, WakeupLevel::Low),
//                 &Ext1WakeupSource::new(
//                     &mut [
//                         &mut self.ext_pins.button_bottom_left,
//                         &mut self.ext_pins.button_top_left,
//                         &mut self.ext_pins.button_top_right,
//                         &mut self.ext_pins.button_bottom_right,
//                     ],
//                     WakeupLevel::High,
//                 ),
//             ],
//             &mut delay,
//         )
//     }
// }
