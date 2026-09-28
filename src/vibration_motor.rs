use embassy_time::{Duration, Timer};
use esp_hal::{gpio::Output, peripherals::GPIO13};

pub struct VibrationMotor<'a> {
    pin: Output<'a>,
}

impl<'a> VibrationMotor<'a> {
    pub fn new(pin: GPIO13<'a>) -> Self {
        VibrationMotor {
            pin: Output::new(pin, esp_hal::gpio::Level::Low, Default::default()),
        }
    }

    pub fn enable(&mut self) {
        self.pin.set_high();
    }

    pub fn disable(&mut self) {
        self.pin.set_low();
    }

    pub async fn vibrate_linear(&mut self, times: u8, interval: Duration) {
        for _ in 0..times.saturating_sub(1) {
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
