#[derive(Debug, Clone, Copy)]
pub enum DurationUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
    Weeks,
    Months,
    Years,
}

impl DurationUnit {
    const SECONDS_PER_MINUTE: u64 = 60;
    const SECONDS_PER_HOUR: u64 = 60 * 60;
    const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
    const SECONDS_PER_WEEK: u64 = 7 * 24 * 60 * 60;
    const SECONDS_PER_MONTH: u64 = 30 * 24 * 60 * 60;
    const SECONDS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
    const NANOS_PER_SECOND: u64 = 1_000_000_000;

    pub fn as_seconds(&self) -> u64 {
        match self {
            DurationUnit::Seconds => 1,
            DurationUnit::Minutes => Self::SECONDS_PER_MINUTE,
            DurationUnit::Hours => Self::SECONDS_PER_HOUR,
            DurationUnit::Days => Self::SECONDS_PER_DAY,
            DurationUnit::Weeks => Self::SECONDS_PER_WEEK,
            DurationUnit::Months => Self::SECONDS_PER_MONTH,
            DurationUnit::Years => Self::SECONDS_PER_YEAR,
        }
    }

    pub fn as_nanoseconds(&self) -> u64 {
        self.as_seconds() * Self::NANOS_PER_SECOND
    }

    pub fn as_milliseconds(&self) -> u64 {
        self.as_seconds() * 1_000
    }

    pub fn as_microseconds(&self) -> u64 {
        self.as_seconds() * 1_000_000
    }
}
