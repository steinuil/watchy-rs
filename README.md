# `watchy-rs`

A pure Rust async firmware for the [SQFMI Watchy](https://watchy.sqfmi.com/) v2!

This project contains a firmware for the Watchy v2, written in Rust using [`esp-hal`](https://github.com/esp-rs/esp-hal) and [`embassy-dev`](https://embassy.dev/).

## Status

The firmware runs on my device and all the device drivers have been tested. At the moment it only displays time, which is only configurable at build time, and battery voltage.

![watchy-rs running on my Watchy](https://sgt.hootr.club/blog/driving-gdeh0154d67-with-rust/new-watchface.jpg)

See the **Plans** and **Contributing** sections for more info.

## Usage

The Watchy v2 uses an ESP32-PICO-D4, which is based on the Xtensa architecture. This target has not been upstreamed into LLVM or rustc, so it requires a more fiddly setup than usual.

### Setup with Nix

The toolchain is packaged in the flake, so you just need to run `nix develop` :)

### Setup without Nix

1. Follow the instructions on [The Rust on ESP Book](https://docs.espressif.com/projects/rust/book/getting-started/toolchain.html) to install the Xtensa toolchain using `espup`.

   1. Install `espup` and use it to install the toolchain.

      ```bash
      cargo install espup --locked
      espup install
      ```

   2. [Export the environment variables](https://github.com/esp-rs/espup?tab=readme-ov-file#environment-variables-setup) in your shell.

      ```bash
      . $HOME/export-esp.sh
      ```

2. Install [`cargo-espflash`](https://github.com/esp-rs/espflash/tree/main/cargo-espflash).

    ```bash
    cargo install cargo-espflash --locked
    ```

### Compiling and flashing

Connect your Watchy, then run this command to compile the firmware and flash it:

```bash
cargo espflash flash --release
```

## Hardware

This crate includes reusable async crates built on top of [embedded-hal-async](https://docs.rs/embedded-hal-async/latest/embedded_hal_async/) for all Watchy v2 peripherals:

- `pcf8563-async`: the PCF8536 real-time clock ([datasheet](https://www.mouser.com/datasheet/2/302/PCF8563-1127619.pdf))
- `bma423-async`: the BMA423 3-axis accelerometer ([datasheet](https://watchy.sqfmi.com/assets/files/BST-BMA423-DS000-1509600-950150f51058597a6234dd3eaafbb1f0.pdf))
- `ssd1681-async`: the SSD1681 display driver ([datasheet](https://www.e-paper-display.com/SSD1681%20V0.13%20Spec903d.pdf)) used by the GDEH0154D67 e-paper display ([datasheet](https://www.e-paper-display.com/GDEH0154D67%20V2.0%20Specificationc58c.pdf))

## Plans

* [ ] Clean up the drivers and release them as crates on `crates.io`
* [ ] Integrate [esp-radio](https://github.com/esp-rs/esp-hal/tree/main/esp-radio) to support Wi-Fi and BLE
* [ ] Release a `watchy_framework` crate to make it easier for other people to build their own watchfaces
* [ ] Create a watchface simulator on top of the framework
* [ ] Support other revisions of the Watchy
  * [ ] Make the Watchy framework crate parametrizable over the peripherals
  * [ ] Watchy v1.0
    * [ ] DS3231 RTC
    * [ ] Button pin layout
    * [ ] Battery ADC pin
  * [ ] Watchy v1.5
    * [ ] Button pin layout
    * [ ] Battery ADC pin
  * [ ] Watchy v3
    * [ ] ESP32-S3 microcontroller
    * [ ] Internal EXT 32Khz Crystal RTC
    * [ ] Display SPI pin setup
      * The display seems to be exactly the same as the v2 in every other way
    * [ ] Button pin layout
    * [ ] Battery ADC pin
    * [ ] Battery voltage divider
    * [ ] Vibration motor pin
    * [ ] I2C pins
    * [ ] Accelerometer INT2 pin
  * [ ] [Yatchy](https://github.com/Szybet/Yatchy)?
* [ ] Support all the features of the C++ Watchy firmware:
  * [x] Display the time
  * [ ] Display a battery percentage
  * [ ] Display a step counter
  * [ ] Configurable hourly buzz
  * [ ] Set the time from the Watchy
  * [ ] Sync time from NTP
  * [ ] Set up wifi AP and credentials
  * [ ] Display weather (not sure I care about this one)
* Other features:
  * [ ] Sleep mode: turn off the minute timer at night, or manually until a button is pressed
  * [ ] Set arbitrary alarms
  * [ ] Pomodoro timer
  * [ ] Notifications and other stuff from a phone through BLE with [Gadgetbridge](https://gadgetbridge.org/)

## Contributing

I'd appreciate (human) help with these things:

* Bugs!
* Supporting other revisions of the Watchy. I'm not planning on buying them so I'm gonna need somebody else to test and/or write the code.
* Modularizing the current code so that it supports said alternate revisions of the Watchy.
* Creating a somewhat configurable "complete" watchface on top of the upcoming `watchy_framework` crate.

Please don't open PRs with descriptions written by AI or with code that you haven't read or don't understand.

## Other firmware

If you're looking for a fully featured firmware that supports other versions of the Watchy you might want to take a look at these other projects:

* [sqfmi/Watchy](https://github.com/sqfmi/Watchy): the original Watchy firmware by SQFMI. You probably know this one.
* [Szybet/InkWatchy](https://github.com/Szybet/InkWatchy): a community firmware with tons of features and games
* [GuruSR/Watchy_GSR](https://github.com/GuruSR/Watchy_GSR): another community firmware with many features
* [fbiego/chronos-watchy](https://github.com/fbiego/chronos-watchy): another community firmware that integrates with a phone app
