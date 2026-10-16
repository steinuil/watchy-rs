use embedded_hal_async::delay::DelayNs;
use esp_hal::{
    analog::adc::{Adc, AdcCalLine, AdcConfig, AdcPin, Attenuation},
    peripherals::{ADC1, GPIO34},
    Blocking,
};

pub struct Battery<'a, Delay> {
    adc: Adc<'a, ADC1<'a>, Blocking>,
    pin: AdcPin<GPIO34<'a>, ADC1<'a>, AdcCalLine<ADC1<'a>>>,
    delay: Delay,
}

impl<'a, Delay: DelayNs> Battery<'a, Delay> {
    const MIN_V: f32 = 3.50;
    const MAX_V: f32 = 4.18;

    // Copied straight from the InfiniTime discharge curve.
    // https://github.com/InfiniTimeOrg/InfiniTime/blob/6c119eb52206b580b556b41633dddc1e1b66a8da/src/components/battery/BatteryController.cpp#L65
    //
    // TODO we need to measure some real readings along with time
    // and recalibrate the curve based on those.
    // The discharge curve in Li-Po batteries is kinda weird
    // because it seems to stay mostly flat with a very sudden
    // drop around 5%
    const DISCHARGE_CURVE: [DischargePercentage; 6] = [
        DischargePercentage::new(Self::MIN_V, 0.0),
        DischargePercentage::new(3.616, 3.0),
        DischargePercentage::new(3.723, 22.0),
        DischargePercentage::new(3.776, 48.0),
        DischargePercentage::new(3.979, 79.0),
        DischargePercentage::new(Self::MAX_V, 100.0),
    ];

    pub fn new(adc: ADC1<'a>, pin: GPIO34<'a>, delay: Delay) -> Self {
        let mut config = AdcConfig::new();
        let pin = config.enable_pin_with_cal(pin, Attenuation::_11dB);
        let adc = Adc::new(adc, config);

        Battery { adc, pin, delay }
    }

    /// Reads the raw calibrated millivolts from the pin.
    async fn read(&mut self) -> u16 {
        loop {
            // This function can only return a nb::Error::WouldBlock
            // so it's safe to ignore the error.
            if let Ok(reading) = self.adc.read_oneshot(&mut self.pin) {
                return reading;
            }

            self.delay.delay_us(2).await;
        }
    }

    pub async fn voltage(&mut self) -> f32 {
        let millivolts = self.read().await;

        // Battery voltage goes through a 1/2 divider.
        millivolts as f32 / 1000.0 * 2.0
    }

    pub async fn percentage(&mut self) -> f32 {
        let voltage = self.voltage().await;

        if voltage <= Self::DISCHARGE_CURVE[0].volts {
            return Self::DISCHARGE_CURVE[0].percent;
        }

        // Ported from InfiniTime's LinearApproximation.
        for [p_prev, p] in Self::DISCHARGE_CURVE.array_windows() {
            if voltage < p_prev.volts {
                return p_prev.percent
                    + (voltage - p_prev.volts) * (p.percent - p_prev.percent)
                        / (p.volts - p_prev.volts);
            }
        }

        Self::DISCHARGE_CURVE[Self::DISCHARGE_CURVE.len() - 1].percent
    }
}

struct DischargePercentage {
    pub volts: f32,
    pub percent: f32,
}

impl DischargePercentage {
    pub const fn new(volts: f32, percent: f32) -> Self {
        Self { volts, percent }
    }
}
