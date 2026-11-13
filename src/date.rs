//! Calendar dates, just enough to say how many years ago something happened.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Date {
    pub(crate) year: i64,
    month: i64,
    day: i64,
}

const SECONDS_PER_DAY: u64 = 86_400;

impl Date {
    pub(crate) fn today() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self::from_days_since_epoch(i64::try_from(seconds / SECONDS_PER_DAY).unwrap_or(0))
    }

    /// Reads the date at the start of an ISO 8601 timestamp such as `2022-05-01T12:00:00.000Z`.
    pub(crate) fn from_timestamp(timestamp: &str) -> Option<Self> {
        let mut parts = timestamp.get(..10)?.split('-').map(str::parse::<i64>);
        let (year, month, day) = (
            parts.next()?.ok()?,
            parts.next()?.ok()?,
            parts.next()?.ok()?,
        );
        let valid = (1..=12).contains(&month) && (1..=31).contains(&day);
        valid.then_some(Self { year, month, day })
    }

    pub(crate) fn whole_years_until(self, later: Self) -> i64 {
        let anniversary_passed = (later.month, later.day) >= (self.month, self.day);
        later.year - self.year - i64::from(!anniversary_passed)
    }

    /// Howard Hinnant's `civil_from_days`, the usual way to turn a day count into a date.
    fn from_days_since_epoch(days: i64) -> Self {
        let shifted = days + 719_468;
        let era = shifted.div_euclid(146_097);
        let day_of_era = shifted.rem_euclid(146_097);
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        Self { year, month, day }
    }
}

#[cfg(test)]
pub(crate) fn date(year: i64, month: i64, day: i64) -> Date {
    Date { year, month, day }
}

#[cfg(test)]
mod tests {
    use super::{Date, date};

    #[test]
    fn day_counts_become_dates() {
        assert_eq!(Date::from_days_since_epoch(0), date(1970, 1, 1));
        assert_eq!(Date::from_days_since_epoch(11_016), date(2000, 2, 29));
        assert_eq!(Date::from_days_since_epoch(20_770), date(2026, 11, 13));
    }

    #[test]
    fn a_timestamp_gives_its_date() {
        assert_eq!(
            Date::from_timestamp("2022-05-01T12:00:00.000Z"),
            Some(date(2022, 5, 1))
        );
    }

    #[test]
    fn something_that_is_not_a_timestamp_gives_nothing() {
        assert_eq!(Date::from_timestamp(""), None);
        assert_eq!(Date::from_timestamp("yesterday at noon"), None);
        assert_eq!(Date::from_timestamp("2022-13-01T00:00:00Z"), None);
    }

    #[test]
    fn years_count_only_once_the_anniversary_has_passed() {
        let published = date(2023, 11, 13);

        assert_eq!(published.whole_years_until(date(2026, 11, 12)), 2);
        assert_eq!(published.whole_years_until(date(2026, 11, 13)), 3);
        assert_eq!(published.whole_years_until(date(2026, 11, 14)), 3);
    }
}
