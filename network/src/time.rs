//! Gregorian UTC conversion and monotonic holdover after a network sync.

use crate::NetworkTime;

/// A broken-down Gregorian UTC value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UtcDateTime {
    /// Gregorian calendar year.
    pub year: i32,
    /// Gregorian month, numbered 1 through 12.
    pub month: u8,
    /// Day of month, numbered 1 through 31.
    pub day: u8,
    /// Hour, numbered 0 through 23.
    pub hour: u8,
    /// Minute, numbered 0 through 59.
    pub minute: u8,
    /// Second, numbered 0 through 59.
    pub second: u8,
}

/// Wall-clock holdover anchored to a platform monotonic clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HoldoverClock {
    base_unix_millis: u64,
    base_monotonic_millis: u64,
}

impl HoldoverClock {
    /// Anchor holdover to an SNTP sample received at `monotonic_millis`.
    ///
    /// Half the measured round trip is added as a symmetric-path estimate.
    pub const fn from_sntp(
        sample: NetworkTime,
        monotonic_millis: u64,
        round_trip_millis: u64,
    ) -> Self {
        Self {
            base_unix_millis: sample.unix_millis.saturating_add(round_trip_millis / 2),
            base_monotonic_millis: monotonic_millis,
        }
    }

    /// Replace the anchor with a newer SNTP sample.
    pub fn resynchronize(
        &mut self,
        sample: NetworkTime,
        monotonic_millis: u64,
        round_trip_millis: u64,
    ) {
        *self = Self::from_sntp(sample, monotonic_millis, round_trip_millis);
    }

    /// Return estimated Unix milliseconds at a monotonic timestamp.
    pub const fn unix_millis_at(self, monotonic_millis: u64) -> u64 {
        self.base_unix_millis
            .saturating_add(monotonic_millis.saturating_sub(self.base_monotonic_millis))
    }

    /// Return whole seconds elapsed since the most recent synchronization.
    pub const fn sync_age_seconds(self, monotonic_millis: u64) -> u64 {
        monotonic_millis.saturating_sub(self.base_monotonic_millis) / 1_000
    }
}

/// Convert seconds since the Unix epoch to Gregorian UTC fields.
pub fn unix_seconds_to_utc(unix_seconds: u64) -> UtcDateTime {
    let seconds_in_day = unix_seconds % 86_400;
    let days = (unix_seconds / 86_400) as i64;

    // Howard Hinnant's civil-from-days algorithm, shifted so day zero is
    // 1970-01-01. All divisions are nonnegative for Unix timestamps.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }

    UtcDateTime {
        year: year as i32,
        month: month as u8,
        day: day as u8,
        hour: (seconds_in_day / 3_600) as u8,
        minute: ((seconds_in_day % 3_600) / 60) as u8,
        second: (seconds_in_day % 60) as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_conversion_handles_epoch_and_leap_day() {
        assert_eq!(
            unix_seconds_to_utc(0),
            UtcDateTime {
                year: 1970,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
            }
        );
        assert_eq!(
            unix_seconds_to_utc(951_827_696),
            UtcDateTime {
                year: 2000,
                month: 2,
                day: 29,
                hour: 12,
                minute: 34,
                second: 56,
            }
        );
    }

    #[test]
    fn holdover_applies_half_rtt_and_monotonic_elapsed_time() {
        let mut clock = HoldoverClock::from_sntp(
            NetworkTime {
                unix_millis: 1_000_000,
                stratum: 2,
            },
            5_000,
            40,
        );
        assert_eq!(clock.unix_millis_at(7_500), 1_002_520);
        assert_eq!(clock.sync_age_seconds(7_500), 2);

        clock.resynchronize(
            NetworkTime {
                unix_millis: 2_000_000,
                stratum: 1,
            },
            8_000,
            10,
        );
        assert_eq!(clock.unix_millis_at(8_100), 2_000_105);
    }
}
