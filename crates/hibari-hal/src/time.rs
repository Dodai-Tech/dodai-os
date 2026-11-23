//! Calendar time, as a real-time clock reports it.

/// A date and time of day, without a time zone. Whatever the clock was set to: usually local time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    /// Year, 2000 to 2099 (what a two-digit clock register can hold).
    pub year: u16,
    /// Month, 1 to 12.
    pub month: u8,
    /// Day of the month, from 1.
    pub day: u8,
    /// Hour, 0 to 23.
    pub hour: u8,
    /// Minute, 0 to 59.
    pub minute: u8,
    /// Second, 0 to 59.
    pub second: u8,
}

/// Whether `year` is a leap year in the Gregorian calendar.
#[must_use]
pub const fn is_leap(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

/// How many days `month` (1 to 12) has in `year`; 0 for a month that does not exist.
#[must_use]
pub const fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

impl DateTime {
    /// Whether every field is in range, including the day for its month and year.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.year >= 2000
            && self.year <= 2099
            && self.day >= 1
            && self.day <= days_in_month(self.year, self.month)
            && self.hour < 24
            && self.minute < 60
            && self.second < 60
    }

    /// The day of the week, 0 for Sunday to 6 for Saturday.
    #[must_use]
    pub const fn weekday(&self) -> u8 {
        // Sakamoto's method.
        const T: [u16; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let m = if self.month >= 1 && self.month <= 12 {
            self.month as usize
        } else {
            1
        };
        let y = if m < 3 { self.year - 1 } else { self.year };
        ((y + y / 4 - y / 100 + y / 400 + T[m - 1] + self.day as u16) % 7) as u8
    }
}

/// A battery-backed clock.
pub trait Rtc {
    /// What a bus or chip failure looks like.
    type Error;

    /// The time, or `None` when the clock lost power and has not been set since, so what it holds
    /// cannot be trusted.
    ///
    /// # Errors
    /// The bus error.
    fn now(&mut self) -> Result<Option<DateTime>, Self::Error>;

    /// Sets the clock.
    ///
    /// # Errors
    /// The bus error. An invalid [`DateTime`] is the caller's mistake and is not written.
    fn set(&mut self, time: &DateTime) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn dt(year: u16, month: u8, day: u8) -> DateTime {
        DateTime {
            year,
            month,
            day,
            hour: 0,
            minute: 0,
            second: 0,
        }
    }

    #[test]
    fn leap_years_follow_the_gregorian_rule() {
        assert!(is_leap(2024) && is_leap(2000));
        assert!(!is_leap(2100) && !is_leap(2023) && !is_leap(1900));
    }

    #[test]
    fn month_lengths_include_february_and_reject_bad_months() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2023, 4), 30);
        assert_eq!(days_in_month(2023, 12), 31);
        assert_eq!(days_in_month(2023, 0), 0);
        assert_eq!(days_in_month(2023, 13), 0);
    }

    #[test]
    fn validity_checks_every_field() {
        assert!(dt(2026, 10, 28).is_valid());
        assert!(dt(2024, 2, 29).is_valid());
        assert!(!dt(2023, 2, 29).is_valid());
        assert!(!dt(2026, 13, 1).is_valid());
        assert!(!dt(2026, 4, 31).is_valid());
        assert!(!dt(2026, 1, 0).is_valid());
        assert!(!dt(1999, 12, 31).is_valid());
        assert!(!dt(2100, 1, 1).is_valid());
        let mut t = dt(2026, 1, 1);
        t.hour = 24;
        assert!(!t.is_valid());
        t.hour = 23;
        t.minute = 60;
        assert!(!t.is_valid());
        t.minute = 59;
        t.second = 60;
        assert!(!t.is_valid());
    }

    #[test]
    fn weekdays_match_known_dates() {
        assert_eq!(dt(2000, 1, 1).weekday(), 6, "Saturday");
        assert_eq!(dt(2024, 2, 29).weekday(), 4, "Thursday");
        assert_eq!(dt(2026, 1, 1).weekday(), 4, "Thursday");
        assert_eq!(dt(2099, 12, 31).weekday(), 4, "Thursday");
        assert_eq!(dt(2023, 10, 1).weekday(), 0, "Sunday");
    }

    #[test]
    fn a_bad_month_does_not_panic_the_weekday() {
        let _ = dt(2026, 0, 1).weekday();
        let _ = dt(2026, 99, 1).weekday();
    }
}
