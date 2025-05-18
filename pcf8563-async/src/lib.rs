#![no_std]

use embedded_hal_async::i2c::I2c;

fn dec_to_bcd(n: u8) -> u8 {
    (n / 10 * 16) + (n % 10)
}

fn bcd_to_dec(n: u8) -> u8 {
    (n / 16 * 10) + (n % 16)
}

// #[test]
// fn test_bcd_to_dec() {
//     assert_eq!(bcd_to_dec(dec_to_bcd(2023)), 2023);
// }

#[derive(Debug)]
pub enum Error<E> {
    Bus(E),
    Time(time::Error),
    InvalidDateTime,
}

impl<E: core::fmt::Display> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Error::Bus(e) => write!(f, "Bus error: {}", e),
            Error::Time(e) => write!(f, "Invalid time: {}", e),
            Error::InvalidDateTime => write!(f, "Invalid time"),
        }
    }
}

impl<E> core::convert::From<E> for Error<E> {
    fn from(error: E) -> Self {
        Error::Bus(error)
    }
}

fn parse_weekday<E>(weekday: u8) -> Result<time::Weekday, Error<E>> {
    match weekday {
        0 => Ok(time::Weekday::Sunday),
        1 => Ok(time::Weekday::Monday),
        2 => Ok(time::Weekday::Tuesday),
        3 => Ok(time::Weekday::Wednesday),
        4 => Ok(time::Weekday::Thursday),
        5 => Ok(time::Weekday::Friday),
        6 => Ok(time::Weekday::Saturday),
        _ => Err(Error::InvalidDateTime),
    }
}

fn parse_date<E>(buf: &[u8]) -> Result<time::Date, Error<E>> {
    let day = bcd_to_dec(buf[0] & mask::DAY);
    let weekday = bcd_to_dec(buf[1] & mask::WEEKDAY);

    let month_bcd = buf[2];
    let year_bcd = buf[3];

    let month = bcd_to_dec(month_bcd & mask::MONTH);
    let year = bcd_to_dec(year_bcd) as i32
        + if month_bcd & mask::CENTURY != 0 {
            1900
        } else {
            2000
        };

    let month = time::Month::try_from(month).map_err(|e| Error::Time(e.into()))?;
    let weekday = parse_weekday(weekday)?;

    let date =
        time::Date::from_calendar_date(year, month, day).map_err(|e| Error::Time(e.into()))?;

    if date.weekday() != weekday {
        return Err(Error::InvalidDateTime);
    }

    Ok(date)
}

fn parse_time<E>(buf: &[u8]) -> Result<time::Time, Error<E>> {
    let second = bcd_to_dec(buf[0] & mask::SECOND);
    let minute = bcd_to_dec(buf[1] & mask::MINUTE);
    let hour = bcd_to_dec(buf[2] & mask::HOUR);

    time::Time::from_hms(hour, minute, second).map_err(|e| Error::Time(e.into()))
}

#[derive(Debug, Clone, Default)]
pub struct AlarmConfig {
    pub minute: Option<u8>,
    pub hour: Option<u8>,
    pub day: Option<u8>,
    pub weekday: Option<time::Weekday>,
}

#[allow(dead_code)]
mod register {
    pub const CONTROL_STATUS_1: u8 = 0x00;
    pub const CONTROL_STATUS_2: u8 = 0x01;
    pub const SECOND: u8 = 0x02;
    pub const MINUTE: u8 = 0x03;
    pub const HOUR: u8 = 0x04;
    pub const DAY: u8 = 0x05;
    pub const ALARM_MINUTE: u8 = 0x09;
    pub const CLOCK_OUTPUT: u8 = 0x0D;
}

#[allow(dead_code)]
mod mask {
    pub const ALARM_FLAG: u8 = 0x08;
    pub const ALARM_INTERRUPT_ENABLED: u8 = 0x02;
    pub const SQUARE_WAVE_ENABLED: u8 = 0x80;

    pub const CENTURY: u8 = 0x80;
    pub const MONTH: u8 = 0b00011111;
    pub const WEEKDAY: u8 = 0b00000111;
    pub const DAY: u8 = 0b00111111;
    pub const HOUR: u8 = 0b00111111;
    pub const MINUTE: u8 = 0b01111111;
    pub const SECOND: u8 = 0b01111111;
}

const ALARM_DISABLED: u8 = 0x80;

pub struct PCF8563<I2C> {
    address: u8,
    i2c: I2C,
}

pub const SLAVE_ADDRESS: u8 = 0x51;

impl<I2C: I2c<Error = E>, E> PCF8563<I2C> {
    pub fn new(address: u8, i2c: I2C) -> PCF8563<I2C> {
        PCF8563 { address, i2c }
    }

    pub async fn reset(&mut self) -> Result<(), Error<E>> {
        self.write(&[
            register::CONTROL_STATUS_1,
            0x00,           // control/status 1
            0x00,           // control/status 2
            0x01,           // second
            0x01,           // minute
            0x01,           // hour
            0x01,           // day
            0x01,           // weekday
            0x01,           // month + century
            0x01,           // year
            ALARM_DISABLED, // minute alarm value reset to 00
            ALARM_DISABLED, // hour alarm value reset to 00
            ALARM_DISABLED, // day alarm value reset to 00
            ALARM_DISABLED, // weekday alarm value reset to 00
            0x00,           // set SQW
            0x00,           // timer off
        ])
        .await
    }

    pub async fn read_date(&mut self) -> Result<time::Date, Error<E>> {
        let mut buf = [0; 4];
        self.read_registers(register::DAY, &mut buf).await?;

        parse_date(&buf)
    }

    pub async fn read_time(&mut self) -> Result<time::Time, Error<E>> {
        let mut buf = [0; 3];
        self.read_registers(register::SECOND, &mut buf).await?;

        parse_time(&buf)
    }

    pub async fn read_datetime(&mut self) -> Result<time::PrimitiveDateTime, Error<E>> {
        let mut buf = [0; 7];
        self.read_registers(register::SECOND, &mut buf).await?;

        let time = parse_time(&buf[0..3])?;
        let date = parse_date(&buf[3..7])?;

        Ok(time::PrimitiveDateTime::new(date, time))
    }

    pub async fn set_time(&mut self, time: time::Time) -> Result<(), Error<E>> {
        self.write(&[
            register::SECOND,
            dec_to_bcd(time.second()),
            dec_to_bcd(time.minute()),
            dec_to_bcd(time.hour()),
        ])
        .await
    }

    pub async fn set_date(&mut self, date: time::Date) -> Result<(), Error<E>> {
        // time::Month is is represented as an u8 with 1-indexed months so we can just
        // call .into().
        let month = date.month().into();

        let month_bcd = if date.year() < 2000 {
            dec_to_bcd(month) | mask::CENTURY
        } else {
            dec_to_bcd(month)
        };

        self.write(&[
            register::DAY,
            dec_to_bcd(date.day()),
            dec_to_bcd(date.weekday().number_days_from_sunday()),
            month_bcd,
            dec_to_bcd((date.year() % 100) as u8),
        ])
        .await
    }

    pub async fn enable_alarm(&mut self) -> Result<(), Error<E>> {
        let mut control_status_2 = self.read_register(register::CONTROL_STATUS_2).await?;
        control_status_2 &= !mask::ALARM_FLAG;
        control_status_2 |= mask::ALARM_INTERRUPT_ENABLED;

        self.write(&[register::CONTROL_STATUS_2, control_status_2])
            .await
    }

    pub async fn disable_alarm(&mut self) -> Result<(), Error<E>> {
        let mut control_status_2 = self.read_register(register::CONTROL_STATUS_2).await?;
        control_status_2 &= !mask::ALARM_INTERRUPT_ENABLED;

        self.write(&[register::CONTROL_STATUS_2, control_status_2])
            .await
    }

    pub async fn is_alarm_enabled(&mut self) -> Result<bool, Error<E>> {
        let control_status_2 = self.read_register(register::CONTROL_STATUS_2).await?;

        Ok(control_status_2 & mask::ALARM_INTERRUPT_ENABLED != 0)
    }

    pub async fn set_alarm(&mut self, alarm: &AlarmConfig) -> Result<(), Error<E>> {
        self.write(&[
            register::ALARM_MINUTE,
            alarm.minute.map_or(ALARM_DISABLED, dec_to_bcd),
            alarm.hour.map_or(ALARM_DISABLED, dec_to_bcd),
            alarm.day.map_or(ALARM_DISABLED, dec_to_bcd),
            alarm
                .weekday
                .map_or(ALARM_DISABLED, |w| dec_to_bcd(w.number_days_from_sunday())),
        ])
        .await
    }

    // async fn clear_control_status(&mut self) -> Result<(), Error<E>> {
    //     self.write(&[register::CONTROL_STATUS_1, 0x00, 0x00]).await
    // }

    async fn read_registers(&mut self, register: u8, buf: &mut [u8]) -> Result<(), Error<E>> {
        self.i2c.write_read(self.address, &[register], buf).await?;
        Ok(())
    }

    async fn read_register(&mut self, register: u8) -> Result<u8, Error<E>> {
        let mut data: [u8; 1] = [0; 1];
        self.read_registers(register, &mut data).await?;
        Ok(data[0])
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.i2c.write(self.address, data).await?;
        Ok(())
    }
}
