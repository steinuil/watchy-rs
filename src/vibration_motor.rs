use embassy_time::{Duration, Timer};
use esp_hal::gpio::Output;

pub struct VibrationMotor<'a> {
    pin: Output<'a>,
}

impl VibrationMotor<'_> {
    pub fn new(pin: esp_hal::gpio::GpioPin<13>) -> Self {
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
