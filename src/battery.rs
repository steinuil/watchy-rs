use esp_hal::{
    analog::adc::{Adc, AdcConfig, AdcPin, Attenuation},
    gpio::GpioPin,
    peripherals::ADC1,
};

pub struct Battery<'a> {
    adc: Adc<'a, ADC1>,
    pin: AdcPin<GpioPin<34>, ADC1>,
}

impl Battery<'_> {
    pub fn new(adc: ADC1, pin: GpioPin<34>) -> Self {
        let mut config = AdcConfig::new();
        let pin = config.enable_pin(pin, Attenuation::_11dB);
        let adc = Adc::new(adc, config);

        Battery { adc, pin }
    }

    pub fn voltage(&mut self) -> Option<f32> {
        // For some reason the first read usually returns WouldBlock
        let _ = self.adc.read_oneshot(&mut self.pin);
        let raw = self.adc.read_oneshot(&mut self.pin).ok()?;
        let voltage = (raw as f32 / 4095.0) * 3.3;
        Some(voltage)
    }
}
