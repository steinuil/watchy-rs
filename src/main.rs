#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_println as _;

mod battery;
pub mod display;
mod vibration_motor;

esp_bootloader_esp_idf::esp_app_desc!();
