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
    for i in 0..99 {
        assert_eq!(bcd_to_dec(dec_to_bcd(i)), i);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error<E> {
    #[error(transparent)]
    Bus(E),

    #[error(
        "invalid datetime: {year}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} (weekday: {weekday})"
    )]
    InvalidDateTime {
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        weekday: u8,
    },

    /// The weekday reported by the RTC is not valid or does not match with the date.
    #[error("invalid time: weekday is invalid or does not match the date")]
    InvalidWeekday,

    /// Clock integrity was compromised due to low voltage being supplied to the chip.
    #[error("clock integrity not guaranteed")]
    ClockIntegrityLost,

    /// This RTC only supports years between 1900 and 2099.
    #[error("year out of supported 1900..=2099 range")]
    YearOutOfRange,

    #[error("alarm components are out of the valid range")]
    InvalidAlarm,
}

fn parse_weekday(weekday: u8) -> Option<time::Weekday> {
    match weekday {
        0 => Some(time::Weekday::Sunday),
        1 => Some(time::Weekday::Monday),
        2 => Some(time::Weekday::Tuesday),
        3 => Some(time::Weekday::Wednesday),
        4 => Some(time::Weekday::Thursday),
        5 => Some(time::Weekday::Friday),
        6 => Some(time::Weekday::Saturday),
        _ => None,
    }
}

fn parse_alarm_component(byte: u8, mask: u8) -> Option<u8> {
    (byte & ALARM_DISABLED == 0).then(|| bcd_to_dec(byte & mask))
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
    pub const TIMER_CONTROL: u8 = 0x0E;
    pub const TIMER_VALUE: u8 = 0x0F;
}

mod mask {
    pub const VOLTAGE_LOW: u8 = 0x80;

    pub const CENTURY: u8 = 0x80;
    pub const MONTH: u8 = 0b00011111;
    pub const WEEKDAY: u8 = 0b00000111;
    pub const DAY: u8 = 0b00111111;
    pub const HOUR: u8 = 0b00111111;
    pub const MINUTE: u8 = 0b01111111;
    pub const SECOND: u8 = 0b01111111;
}

const ALARM_DISABLED: u8 = 0x80;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct ControlStatus2: u8 {
        const TIMER_INTERRUPT_ENABLED = 1;
        const ALARM_INTERRUPT_ENABLED = 1 << 1;
        const TIMER_FLAG = 1 << 2;
        const ALARM_FLAG = 1 << 3;
        const TIMER_PULSES = 1 << 4;
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerFrequency {
    _4096Hz = 0b00,
    _64Hz = 0b01,
    _1Hz = 0b10,
    _1_60thHz = 0b11,
}

const TIMER_ENABLED: u8 = 0x80;
const TIMER_DISABLED: u8 = TimerFrequency::_1_60thHz as u8;

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
            0x00,           // CLKOUT off
            TIMER_DISABLED, // timer off
        ])
        .await
    }

    pub async fn is_clock_integrity_guaranteed(&mut self) -> Result<bool, Error<E>> {
        let seconds = self.read_register(register::SECOND).await?;

        Ok(seconds & mask::VOLTAGE_LOW == 0)
    }

    /// Reads the current date/time.
    ///
    /// Returns an [`Error::InvalidDateTime`] containing the values returned by
    /// the RTC when one of the components is invalid.
    pub async fn datetime(&mut self) -> Result<time::UtcDateTime, Error<E>> {
        let mut buf = [0; 7];
        self.read_registers(register::SECOND, &mut buf).await?;

        if buf[0] & mask::VOLTAGE_LOW != 0 {
            return Err(Error::ClockIntegrityLost);
        }

        let second = bcd_to_dec(buf[0] & mask::SECOND);
        let minute = bcd_to_dec(buf[1] & mask::MINUTE);
        let hour = bcd_to_dec(buf[2] & mask::HOUR);

        let day = bcd_to_dec(buf[3] & mask::DAY);
        let weekday = bcd_to_dec(buf[4] & mask::WEEKDAY);

        let month_bcd = buf[5];
        let month = bcd_to_dec(month_bcd & mask::MONTH);
        let year = bcd_to_dec(buf[6]) as i32
            + if month_bcd & mask::CENTURY != 0 {
                1900
            } else {
                2000
            };

        let invalid = || Error::<E>::InvalidDateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            weekday,
        };

        let time = time::Time::from_hms(hour, minute, second).map_err(|_| invalid())?;

        let month = time::Month::try_from(month).map_err(|_| invalid())?;
        let weekday = parse_weekday(weekday).ok_or_else(invalid)?;

        let date = time::Date::from_calendar_date(year, month, day).map_err(|_| invalid())?;

        if date.weekday() != weekday {
            return Err(invalid());
        }

        Ok(time::UtcDateTime::new(date, time))
    }

    pub async fn set_datetime(&mut self, datetime: time::UtcDateTime) -> Result<(), Error<E>> {
        let date = datetime.date();
        let time = datetime.time();

        if !(1900..2099).contains(&date.year()) {
            return Err(Error::YearOutOfRange);
        }

        // time::Month is is represented as an u8 with 1-indexed months so we can just
        // call .into().
        let month = date.month().into();

        let month_bcd = if date.year() < 2000 {
            dec_to_bcd(month) | mask::CENTURY
        } else {
            dec_to_bcd(month)
        };

        self.write(&[
            register::SECOND,
            dec_to_bcd(time.second()),
            dec_to_bcd(time.minute()),
            dec_to_bcd(time.hour()),
            dec_to_bcd(date.day()),
            dec_to_bcd(date.weekday().number_days_from_sunday()),
            month_bcd,
            dec_to_bcd((date.year() % 100) as u8),
        ])
        .await
    }

    pub async fn enable_alarm(&mut self) -> Result<(), Error<E>> {
        self.modify_control_status_2(|r| {
            r.difference(ControlStatus2::ALARM_FLAG)
                .union(ControlStatus2::ALARM_INTERRUPT_ENABLED)
        })
        .await
    }

    pub async fn disable_alarm(&mut self) -> Result<(), Error<E>> {
        self.modify_control_status_2(|r| r.difference(ControlStatus2::ALARM_INTERRUPT_ENABLED))
            .await
    }

    pub async fn is_alarm_enabled(&mut self) -> Result<bool, Error<E>> {
        Ok(self
            .control_status_2()
            .await?
            .contains(ControlStatus2::ALARM_INTERRUPT_ENABLED))
    }

    pub async fn clear_alarm_flag(&mut self) -> Result<(), Error<E>> {
        self.modify_control_status_2(|r| r.difference(ControlStatus2::ALARM_FLAG))
            .await
    }

    pub async fn alarm(&mut self) -> Result<AlarmConfig, Error<E>> {
        let mut buf = [0; 4];
        self.read_registers(register::ALARM_MINUTE, &mut buf)
            .await?;

        Ok(AlarmConfig {
            minute: parse_alarm_component(buf[0], mask::MINUTE),
            hour: parse_alarm_component(buf[1], mask::HOUR),
            day: parse_alarm_component(buf[2], mask::DAY),
            weekday: parse_alarm_component(buf[3], mask::WEEKDAY)
                .map(|w| parse_weekday(w).ok_or(Error::InvalidWeekday))
                .transpose()?,
        })
    }

    pub async fn set_alarm(&mut self, alarm: &AlarmConfig) -> Result<(), Error<E>> {
        let valid = alarm.minute.is_none_or(|m| m < 60)
            && alarm.hour.is_none_or(|h| h < 24)
            && alarm.day.is_none_or(|d| (1..=31).contains(&d));
        if !valid {
            return Err(Error::InvalidAlarm);
        }

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

    pub async fn set_timer(
        &mut self,
        frequency: TimerFrequency,
        ticks: u8,
    ) -> Result<(), Error<E>> {
        self.write(&[
            register::TIMER_CONTROL,
            TIMER_ENABLED | frequency as u8,
            ticks,
        ])
        .await
    }

    pub async fn disable_timer(&mut self) -> Result<(), Error<E>> {
        self.write(&[register::TIMER_CONTROL, TIMER_DISABLED]).await
    }

    pub async fn clear_timer_flag(&mut self) -> Result<(), Error<E>> {
        self.modify_control_status_2(|r| r.difference(ControlStatus2::TIMER_FLAG))
            .await
    }

    pub async fn enable_timer_interrupt(&mut self) -> Result<(), Error<E>> {
        self.modify_control_status_2(|r| {
            r.difference(ControlStatus2::TIMER_FLAG)
                .union(ControlStatus2::TIMER_INTERRUPT_ENABLED)
        })
        .await
    }

    async fn control_status_2(&mut self) -> Result<ControlStatus2, Error<E>> {
        let control_status_2 = self.read_register(register::CONTROL_STATUS_2).await?;

        Ok(ControlStatus2::from_bits_truncate(control_status_2))
    }

    async fn modify_control_status_2<F>(&mut self, f: F) -> Result<(), Error<E>>
    where
        F: FnOnce(ControlStatus2) -> ControlStatus2,
    {
        // We set the timer and alarm flags before writing because, according to the datasheet,
        // a value of 1 in these bits is ignored while writing to C/S2 to avoid a race condition.
        //
        // Example:
        // 1. We read C/S2 with the alarm flag to 0
        // 2. The alarm flag is raised
        // 3. We write to C/S2 with the alarm flag as it was previously,
        //    accidentally clearing the flag in the process.
        let current = self.control_status_2().await?
            | ControlStatus2::ALARM_FLAG
            | ControlStatus2::TIMER_FLAG;

        self.write(&[register::CONTROL_STATUS_2, f(current).bits()])
            .await
    }

    async fn read_registers(&mut self, register: u8, buf: &mut [u8]) -> Result<(), Error<E>> {
        self.i2c
            .write_read(self.address, &[register], buf)
            .await
            .map_err(Error::Bus)?;
        Ok(())
    }

    async fn read_register(&mut self, register: u8) -> Result<u8, Error<E>> {
        let mut data: [u8; 1] = [0; 1];
        self.read_registers(register, &mut data).await?;
        Ok(data[0])
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.i2c
            .write(self.address, data)
            .await
            .map_err(Error::Bus)?;
        Ok(())
    }
}
