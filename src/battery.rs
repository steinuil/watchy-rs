use embedded_hal_async::delay::DelayNs;
use esp_hal::{
    analog::adc::{Adc, AdcConfig, AdcPin, Attenuation},
    gpio::GpioPin,
    peripherals::ADC1,
};

pub struct Battery<'a, Delay> {
    adc: Adc<'a, ADC1>,
    pin: AdcPin<GpioPin<34>, ADC1>,
    delay: Delay,
}

impl<Delay: DelayNs> Battery<'_, Delay> {
    pub fn new(adc: ADC1, pin: GpioPin<34>, delay: Delay) -> Self {
        let mut config = AdcConfig::new();
        let pin = config.enable_pin(pin, Attenuation::_11dB);
        let adc = Adc::new(adc, config);

        Battery { adc, pin, delay }
    }

    pub async fn read(&mut self) -> u16 {
        loop {
            // This function can only return a nb::Error::WouldBlock
            // so it's safe to ignore the error.
            if let Ok(reading) = self.adc.read_oneshot(&mut self.pin) {
                return reading;
            }

            self.delay.delay_us(2).await;
        }
    }

    // TODO this doesn't seem to be correct. Check the battery
    pub async fn voltage(&mut self) -> f32 {
        let reading = self.read().await;

        let voltage_adc = (reading as f32 / 4095.0) * 3.3;

        // Battery voltage goes through a 1/2 divider.
        voltage_adc * 2.0
    }
}
