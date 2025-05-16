use bitflags::bitflags;
use esp_hal::peripherals::LPWR;

bitflags! {
    pub struct WakeupButtons : u32 {
        const TOP_RIGHT = 1 << 5;
        const TOP_LEFT = 1 << 6;
        const BOTTOM_LEFT = 1 << 7;
        const BOTTOM_RIGHT = 1 << 10;
    }
}

impl WakeupButtons {
    pub fn from_wakeup_status(rtc_cntl: &LPWR) -> Self {
        let wakeup_bits = rtc_cntl.ext_wakeup1_status().read().bits();

        WakeupButtons::from_bits_retain(wakeup_bits)
    }
}
