//! Host wall-clock time, supplied by the frontend (the core never reads the host clock).

/// A calendar date and time of day, local time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WallTime {
    pub year: u16,
    /// 1-12.
    pub month: u8,
    /// 1-31.
    pub day: u8,
    /// 0 = Sunday ... 6 = Saturday (`tm_wday`, SYSTEMTIME `wDayOfWeek`).
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    /// 0-99.
    pub centisecond: u8,
}

/// Source of the current local time for the clock cards.
pub trait HostClock {
    fn now(&self) -> WallTime;
}

/// A clock that always reads the same time (tests, deterministic runs).
#[derive(Clone, Copy, Debug)]
pub struct FixedClock(pub WallTime);

impl HostClock for FixedClock {
    fn now(&self) -> WallTime {
        self.0
    }
}

impl WallTime {
    /// 2026-10-19 (a Monday) 14:35:27.12, handy for tests.
    pub const SAMPLE: WallTime = WallTime { year: 2026, month: 10, day: 19, weekday: 1, hour: 14, minute: 35, second: 27, centisecond: 12 };
}

/// `Clock_Util_ConvertTimeToProdos` (Peripheral_Clock_Generic.cpp:96): the four ProDOS
/// date/time bytes for `$BF90-$BF93`.
pub fn prodos_datetime(t: &WallTime) -> [u8; 4] {
    let month = t.month & 0x0F;
    [
        (t.day & 0x1F) | ((u16::from(month) << 5) as u8),
        (((t.year % 100) as u8) << 1) | ((month >> 3) & 1),
        t.minute & 0x3F,
        t.hour & 0x1F,
    ]
}

/// `Clock_Generic_UpdateProDos` (:117): writes the time into ProDOS's global page, `$BF90-$BF93`
/// of main memory.
pub fn update_prodos_clock(m: &mut apple2_core::Machine, clock: &dyn HostClock) {
    for (i, b) in prodos_datetime(&clock.now()).into_iter().enumerate() {
        m.poke_main(0xBF90 + i as u16, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prodos_bytes_pack_date_and_time() {
        // 2026-10-19 14:35: day 19, month 10, year 26 -> %0011010 (7 bits) with month bit 3.
        let b = prodos_datetime(&WallTime::SAMPLE);
        assert_eq!(b[0], 19 | 0x40);
        assert_eq!(b[1], (26 << 1) | 1);
        assert_eq!((b[2], b[3]), (35, 14));
    }
}
