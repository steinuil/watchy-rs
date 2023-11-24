## esp32-hal documentation

- https://docs.rs/esp32-hal/latest/
- https://docs.rs/esp32/latest/

## Data sheets and crates

https://watchy.sqfmi.com/docs/hardware

- Microcontroller ESP32-PICO-D4: esp-idf-hal ?
- E-Paper Display GDEH0154D67
  - https://docs.rs/gdeh0154d67/latest/gdeh0154d67/
  - https://gitlab.com/CasalI/gdeh0154d67
  - https://github.com/ZinggJM/GxEPD/blob/master/src/GxGDEH0154D67/GxGDEH0154D67.cpp
- Real time clock PCF8563: https://docs.rs/pcf8563/latest/pcf8563/
- 3-axis accelerometer BMA423: https://docs.rs/bma423/latest/bma423/

## esp toolchain on Nix

- https://github.com/sdobz/rust-esp-nix (3 years old)
- https://github.com/kate-shine/nix-esp32 (uses the docker image)
- https://n8henrie.com/2023/09/compiling-rust-for-the-esp32-with-nix/ (doesn't support xtensa architectures)

## TODO

- BMA423: add states for initialized/fullpower/powersave like in the non-async crate
- GDEH0154D67
- PCF8563
