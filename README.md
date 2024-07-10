## esp32-hal documentation

- https://docs.rs/esp32-hal/latest/
- https://docs.rs/esp32/latest/

## Data sheets and crates

https://watchy.sqfmi.com/docs/hardware

- Microcontroller [ESP32-PICO-D4](https://www.espressif.com/sites/default/files/documentation/esp32-pico-d4_datasheet_en.pdf)
- E-Paper Display [GDEH0154D67](https://www.e-paper-display.com/GDEH0154D67%20V2.0%20Specificationc58c.pdf) [SSD1681](https://www.e-paper-display.com/SSD1681%20V0.13%20Spec903d.pdf)
  - https://docs.rs/gdeh0154d67/latest/gdeh0154d67/
  - https://gitlab.com/CasalI/gdeh0154d67
  - https://github.com/ZinggJM/GxEPD/blob/master/src/GxGDEH0154D67/GxGDEH0154D67.cpp
- Real time clock [PCF8563](https://www.mouser.com/datasheet/2/302/PCF8563-1127619.pdf)
  - https://docs.rs/pcf8563/latest/pcf8563/
- 3-axis accelerometer [BMA423](https://watchy.sqfmi.com/assets/files/BST-BMA423-DS000-1509600-950150f51058597a6234dd3eaafbb1f0.pdf)
  - https://docs.rs/bma423/latest/bma423/

## esp toolchain on Nix

- https://github.com/sdobz/rust-esp-nix (3 years old)
- https://github.com/kate-shine/nix-esp32 (uses the docker image)
- https://n8henrie.com/2023/09/compiling-rust-for-the-esp32-with-nix/ (doesn't support xtensa architectures)

## TODO

- BMA423
  - test on the Watchy
  - add states for initialized/fullpower/powersave like in the non-async crate
- GDEH0154D67:
  - partial updates
  - lots of configurability stuff
  - could try doing grayscale by manipulating the border waveform
    - https://hackaday.io/project/11537-nekocal-an-e-ink-calendar/log/72153-can-you-get-32-level-grayscale-out-of-an-e-ink-display
    - https://github.com/zkarcher/FancyEPD
- PCF8563
  - better error handling
  - functionality missing
  - provide the raw time/date numbers from the registers and add a `time` feature that enables support for the `time` crate
- Wi-fi and BLE: https://github.com/esp-rs/esp-wifi

Have a look, y'all: https://github.com/sqfmi/Watchy/pull/242
