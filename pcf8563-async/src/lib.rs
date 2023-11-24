#![no_std]

use embedded_hal_async::i2c::I2c;

fn dec_to_bcd(n: u8) -> u8 {
    (n / 10 * 16) + (n % 10)
}

fn bcd_to_dec(n: u8) -> u8 {
    (n / 16 * 10) + (n % 16)
}

#[test]
fn test_bcd_to_dec() {
    assert_eq!(bcd_to_dec(dec_to_bcd(2023)), 2023);
}

pub struct PCF8563<I2C> {
    address: u8,
    i2c: I2C,
}

mod register {
    pub const CONTROL_STATUS_1: u8 = 0x00;
    pub const CONTROL_STATUS_2: u8 = 0x01;
}

impl<I2C: I2c<Error = E>, E> PCF8563<I2C> {
    pub fn new(address: u8, i2c: I2C) -> Self {
        PCF8563 { address, i2c }
    }

    pub async fn initialize(&mut self) -> Result<(), E> {
        self.write(&[
            register::CONTROL_STATUS_1,
            0x00, // control/status 1
            0x00, // control/status 2
            0x01, // second
            0x01, // minute
            0x01, // hour
            0x01, // day
            0x01, // weekday
            0x01, // month + century
            0x01, // year
            0x80, // minute alarm value reset to 00
            0x80, // hour alarm value reset to 00
            0x80, // day alarm value reset to 00
            0x80, // weekday alarm value reset to 00
            0x00, // set SQW
            0x00, // timer off
        ])
        .await
    }

    async fn read_registers(&mut self, register: u8, buf: &mut [u8]) -> Result<(), E> {
        self.i2c.write_read(self.address, &[register], buf).await
    }

    async fn read_u8(&mut self, register: u8) -> Result<u8, E> {
        let mut data: [u8; 1] = [0; 1];
        self.read_registers(register, &mut data).await?;
        Ok(data[0])
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), E> {
        self.i2c.write(self.address, data).await
    }
}
