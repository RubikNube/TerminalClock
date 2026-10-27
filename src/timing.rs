use chrono::{DateTime, LocalResult, NaiveTime, TimeZone, Utc};

use std::time::{Duration, Instant};

pub fn parse_duration(input: &str) -> Result<Duration, String> {
    let [hours, minutes, seconds] = parse_duration_fields(input)?;
    let seconds = hours * 3600 + minutes * 60 + seconds;
    if seconds == 0 {
        return Err("Duration must be 00:00:01 through 99:59:59".into());
    }
    Ok(Duration::from_secs(seconds))
}

/// Parse editable fields, allowing zero while the user adjusts a duration.
pub fn parse_duration_fields(input: &str) -> Result<[u64; 3], String> {
    let parts: Vec<_> = input.split(':').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.len() != 2 || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("Enter a complete HH:MM:SS value before adjusting or saving".into());
    }
    let numbers: Vec<u64> = parts.iter().map(|p| p.parse().unwrap()).collect();
    if numbers[1] > 59 || numbers[2] > 59 {
        return Err("Hours must be 00–99; minutes and seconds must be 00–59".into());
    }
    Ok([numbers[0], numbers[1], numbers[2]])
}

pub fn format_duration(duration: Duration, hundredths: bool, round_up: bool) -> String {
    let seconds = duration.as_secs() + u64::from(round_up && duration.subsec_nanos() > 0);
    let base = format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    );
    if hundredths {
        format!("{base}.{:02}", duration.subsec_millis() / 10)
    } else {
        base
    }
}

/// An elapsed-time accumulator shared by both timing tools. Drawing never advances it.
#[derive(Default)]
pub struct Elapsed {
    accumulated: Duration,
    started: Option<Instant>,
}

impl Elapsed {
    pub fn value(&self, now: Instant) -> Duration {
        self.accumulated
            + self
                .started
                .map_or(Duration::ZERO, |start| now.duration_since(start))
    }

    pub fn running(&self) -> bool {
        self.started.is_some()
    }

    pub fn toggle(&mut self, now: Instant) {
        if self.running() {
            self.accumulated = self.value(now);
            self.started = None;
        } else {
            self.started = Some(now);
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub struct Timer {
    pub duration: Duration,
    elapsed: Elapsed,
    pub finished: bool,
    pub notice: bool,
    last_bell: Option<Instant>,
}

impl Timer {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            elapsed: Elapsed::default(),
            finished: false,
            notice: false,
            last_bell: None,
        }
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        self.duration.saturating_sub(self.elapsed.value(now))
    }

    pub fn running(&self) -> bool {
        self.elapsed.running()
    }

    pub fn editable(&self, now: Instant) -> bool {
        self.finished || (!self.running() && self.elapsed.value(now).is_zero())
    }

    pub fn reset(&mut self) {
        self.elapsed.reset();
        self.finished = false;
        self.notice = false;
        self.last_bell = None;
    }

    pub fn toggle(&mut self, now: Instant) {
        if self.finished {
            self.reset();
        }
        self.elapsed.toggle(now);
    }

    /// Returns true exactly once per expiration.
    pub fn tick(&mut self, now: Instant) -> bool {
        if self.running() && self.remaining(now).is_zero() {
            self.elapsed.toggle(now);
            self.finished = true;
            self.notice = true;
            self.last_bell = None;
            true
        } else {
            false
        }
    }

    /// Ring immediately on completion, then at most once per second until dismissed.
    /// Schedule from the current time to avoid a burst after a delayed update.
    pub fn bell_due(&mut self, now: Instant, repeat: bool) -> bool {
        if !self.notice {
            return false;
        }
        if self
            .last_bell
            .is_none_or(|last| repeat && now.duration_since(last) >= Duration::from_secs(1))
        {
            self.last_bell = Some(now);
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
pub struct Stopwatch {
    pub elapsed: Elapsed,
    pub laps: Vec<Lap>,
    /// Index in newest-first presentation order.
    pub selected: usize,
}

pub struct Lap {
    pub total: Duration,
    pub split: Duration,
}

impl Stopwatch {
    pub fn lap(&mut self, now: Instant) {
        if !self.elapsed.running() {
            return;
        }
        let total = self.elapsed.value(now);
        let previous = self.laps.last().map_or(Duration::ZERO, |lap| lap.total);
        self.laps.push(Lap {
            total,
            split: total - previous,
        });
        self.selected = 0;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_validation_and_formatting() {
        for bad in [
            "00:00:00",
            "1:00:00",
            "00:60:00",
            "00:00:60",
            "-1:00:00",
            "aa:00:00",
            "100:00:00",
        ] {
            assert!(parse_duration(bad).is_err(), "{bad}");
        }
        assert_eq!(parse_duration("99:59:59").unwrap().as_secs(), 359999);
        assert_eq!(
            format_duration(Duration::from_millis(1), false, true),
            "00:00:01"
        );
        assert_eq!(format_duration(Duration::ZERO, false, true), "00:00:00");
        assert_eq!(
            format_duration(Duration::from_millis(61234), true, false),
            "00:01:01.23"
        );
    }

    #[test]
    fn countdown_pauses_expires_once_and_restarts() {
        let t = Instant::now();
        let mut timer = Timer::new(Duration::from_secs(5));
        assert!(timer.editable(t));
        timer.toggle(t);
        timer.toggle(t + Duration::from_secs(2));
        assert_eq!(
            timer.remaining(t + Duration::from_secs(100)),
            Duration::from_secs(3)
        );
        assert!(!timer.editable(t + Duration::from_secs(100)));
        timer.toggle(t + Duration::from_secs(100));
        assert!(!timer.tick(t + Duration::from_secs(102)));
        assert!(timer.tick(t + Duration::from_secs(104)));
        assert!(!timer.tick(t + Duration::from_secs(105)));
        assert!(timer.finished && timer.notice && !timer.running());
        timer.toggle(t + Duration::from_secs(106));
        assert!(!timer.finished && !timer.notice && timer.running());
        timer.reset();
        assert_eq!(
            timer.remaining(t + Duration::from_secs(107)),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn stopwatch_laps_exclude_paused_time() {
        let t = Instant::now();
        let mut watch = Stopwatch::default();
        watch.lap(t);
        assert!(watch.laps.is_empty());
        watch.elapsed.toggle(t);
        watch.lap(t + Duration::from_secs(2));
        watch.elapsed.toggle(t + Duration::from_secs(3));
        watch.elapsed.toggle(t + Duration::from_secs(10));
        watch.lap(t + Duration::from_secs(12));
        assert_eq!(watch.laps[1].total, Duration::from_secs(5));
        assert_eq!(watch.laps[1].split, Duration::from_secs(3));
        watch.reset();
        assert!(watch.laps.is_empty());
        assert_eq!(
            watch.elapsed.value(t + Duration::from_secs(20)),
            Duration::ZERO
        );
    }

    #[test]
    fn repeating_bell_stops_on_dismiss_reset_and_restart() {
        let start = Instant::now();
        let mut timer = Timer::new(Duration::from_secs(1));
        assert!(!timer.bell_due(start, true));
        timer.toggle(start);
        let end = start + Duration::from_secs(1);
        timer.tick(end);
        assert!(timer.bell_due(end, true));
        assert!(!timer.bell_due(end, true));
        assert!(!timer.bell_due(end + Duration::from_millis(999), true));
        assert!(timer.bell_due(end + Duration::from_secs(1), true));
        // Delayed updates ring once without catching up missed repetitions.
        assert!(timer.bell_due(end + Duration::from_secs(20), true));
        assert!(!timer.bell_due(end + Duration::from_secs(20), true));
        timer.notice = false;
        assert!(!timer.bell_due(end + Duration::from_secs(21), true));
        timer.toggle(end + Duration::from_secs(22));
        assert!(!timer.bell_due(end + Duration::from_secs(22), true));
        timer.tick(end + Duration::from_secs(23));
        assert!(timer.bell_due(end + Duration::from_secs(23), true));
        timer.reset();
        assert!(!timer.bell_due(end + Duration::from_secs(24), true));
    }

    #[test]
    fn single_bell_mode_rings_once_per_completion() {
        let start = Instant::now();
        let mut timer = Timer::new(Duration::from_secs(1));
        timer.toggle(start);
        let end = start + Duration::from_secs(1);
        timer.tick(end);
        assert!(timer.bell_due(end, false));
        assert!(!timer.bell_due(end + Duration::from_secs(100), false));
        assert!(timer.notice);
    }
}

/// Alarm input always uses 24-hour time, independent of clock display settings.
pub fn parse_alarm_time(input: &str) -> Result<NaiveTime, String> {
    let bytes = input.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 2 || b.is_ascii_digit())
    {
        return Err("Enter a complete HH:MM time (00:00 through 23:59)".into());
    }
    NaiveTime::from_hms_opt(input[..2].parse().unwrap(), input[3..].parse().unwrap(), 0)
        .ok_or_else(|| "Hours must be 00–23; minutes must be 00–59".into())
}

/// Resolve each local date separately to handle missing and repeated local times.
pub fn next_alarm<T: TimeZone>(
    time: NaiveTime,
    now: &DateTime<T>,
) -> Result<DateTime<Utc>, String> {
    let mut date = now.date_naive();
    loop {
        let candidates = match now.timezone().from_local_datetime(&date.and_time(time)) {
            LocalResult::Single(value) => vec![value],
            LocalResult::Ambiguous(a, b) => vec![a, b],
            LocalResult::None => vec![],
        };
        if let Some(next) = candidates.into_iter().filter(|value| value > now).min() {
            return Ok(next.with_timezone(&Utc));
        }
        date = date
            .succ_opt()
            .ok_or("Cannot schedule an alarm beyond the supported date range")?;
    }
}

pub struct Alarm {
    pub time: NaiveTime,
    pub target: Option<DateTime<Utc>>,
    pub notice: bool,
    last_bell: Option<Instant>,
}

impl Alarm {
    pub fn new(time: NaiveTime) -> Self {
        Self {
            time,
            target: None,
            notice: false,
            last_bell: None,
        }
    }

    pub fn reset(&mut self) {
        self.target = None;
        self.notice = false;
        self.last_bell = None;
    }

    pub fn toggle<T: TimeZone>(&mut self, now: &DateTime<T>) -> Result<(), String> {
        if self.target.is_some() {
            self.reset();
        } else {
            let target = next_alarm(self.time, now)?;
            self.reset();
            self.target = Some(target);
        }
        Ok(())
    }

    pub fn tick(&mut self, now: DateTime<Utc>) -> bool {
        if self.target.is_some_and(|target| now >= target) {
            self.target = None;
            self.notice = true;
            self.last_bell = None;
            true
        } else {
            false
        }
    }

    pub fn bell_due(&mut self, now: Instant, repeat: bool) -> bool {
        if self.notice
            && self
                .last_bell
                .is_none_or(|last| repeat && now.duration_since(last) >= Duration::from_secs(1))
        {
            self.last_bell = Some(now);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod alarm_tests {
    use super::*;
    use chrono::{FixedOffset, NaiveDate, NaiveDateTime};

    fn utc(value: &str) -> DateTime<Utc> {
        value.parse().unwrap()
    }

    #[test]
    fn alarm_time_validation_and_next_occurrence() {
        for bad in [
            "7:30", "24:00", "12:60", "07:30:00", " 07:30", "aa:bb", "é:00", "",
        ] {
            assert!(parse_alarm_time(bad).is_err(), "{bad}");
        }
        let midnight = parse_alarm_time("00:00").unwrap();
        assert_eq!(
            next_alarm(midnight, &utc("2024-01-01T23:59:00Z")).unwrap(),
            utc("2024-01-02T00:00:00Z")
        );
        assert_eq!(
            next_alarm(midnight, &utc("2024-01-02T00:00:00Z")).unwrap(),
            utc("2024-01-03T00:00:00Z")
        );
        assert!(parse_alarm_time("23:59").is_ok());
        let zone = FixedOffset::east_opt(3600).unwrap();
        let now = utc("2024-01-01T05:00:00Z").with_timezone(&zone);
        assert_eq!(
            next_alarm(parse_alarm_time("07:00").unwrap(), &now).unwrap(),
            utc("2024-01-01T06:00:00Z")
        );
    }

    #[test]
    fn alarm_triggers_once_after_delays_and_can_be_rearmed() {
        let start = utc("2024-01-01T06:00:00Z");
        let mut alarm = Alarm::new(parse_alarm_time("07:00").unwrap());
        alarm.toggle(&start).unwrap();
        let target = alarm.target;
        assert!(!alarm.tick(start - chrono::Duration::hours(1)));
        assert_eq!(alarm.target, target);
        assert!(!alarm.tick(start + chrono::Duration::minutes(59)));
        assert!(alarm.tick(start + chrono::Duration::hours(2)));
        assert!(alarm.notice && alarm.target.is_none());
        assert!(!alarm.tick(start + chrono::Duration::hours(3)));
        alarm.toggle(&(start + chrono::Duration::hours(3))).unwrap();
        assert!(!alarm.notice);
        assert_eq!(alarm.target.unwrap(), utc("2024-01-02T07:00:00Z"));
        alarm.toggle(&start).unwrap();
        assert!(alarm.target.is_none());
        assert!(!alarm.tick(utc("2024-01-03T00:00:00Z")));
    }

    #[test]
    fn alarm_bells_repeat_or_ring_once_and_stop_on_dismissal_or_reset() {
        let start = utc("2024-01-01T06:00:00Z");
        let now = Instant::now();
        for repeat in [false, true] {
            let mut alarm = Alarm::new(parse_alarm_time("07:00").unwrap());
            alarm.toggle(&start).unwrap();
            assert!(!alarm.bell_due(now, repeat));
            alarm.tick(start + chrono::Duration::hours(1));
            assert!(alarm.bell_due(now, repeat));
            assert!(!alarm.bell_due(now + Duration::from_millis(999), repeat));
            assert_eq!(
                alarm.bell_due(now + Duration::from_secs(20), repeat),
                repeat
            );
            assert!(!alarm.bell_due(now + Duration::from_secs(20), repeat));
            alarm.notice = false;
            assert!(!alarm.bell_due(now + Duration::from_secs(21), repeat));
            alarm.reset();
            assert!(!alarm.notice && alarm.target.is_none());
        }
    }

    // Deterministic timezone with Europe's 2024 spring gap and autumn overlap.
    // Avoid changing process-global TZ or depending on the host's timezone database.
    #[derive(Clone)]
    struct TestZone;

    fn naive(value: &str) -> NaiveDateTime {
        value.parse().unwrap()
    }
    fn offset(hours: i32) -> FixedOffset {
        FixedOffset::east_opt(hours * 3600).unwrap()
    }

    impl TimeZone for TestZone {
        type Offset = FixedOffset;
        fn from_offset(_: &FixedOffset) -> Self {
            Self
        }
        fn offset_from_local_date(&self, date: &NaiveDate) -> LocalResult<FixedOffset> {
            self.offset_from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
        }
        fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<FixedOffset> {
            if *local < naive("2024-03-31T02:00:00") || *local >= naive("2024-10-27T03:00:00") {
                LocalResult::Single(offset(1))
            } else if *local < naive("2024-03-31T03:00:00") {
                LocalResult::None
            } else if *local >= naive("2024-10-27T02:00:00") {
                // Intentionally put the later occurrence first.
                LocalResult::Ambiguous(offset(1), offset(2))
            } else {
                LocalResult::Single(offset(2))
            }
        }
        fn offset_from_utc_date(&self, date: &NaiveDate) -> FixedOffset {
            self.offset_from_utc_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
        }
        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            if *utc >= naive("2024-03-31T01:00:00") && *utc < naive("2024-10-27T01:00:00") {
                offset(2)
            } else {
                offset(1)
            }
        }
    }

    #[test]
    fn skips_missing_times_and_selects_first_future_ambiguous_occurrence() {
        let time = parse_alarm_time("02:30").unwrap();
        for (now, expected) in [
            ("2024-03-30T23:00:00Z", "2024-04-01T00:30:00Z"),
            ("2024-10-26T23:00:00Z", "2024-10-27T00:30:00Z"),
            ("2024-10-27T00:45:00Z", "2024-10-27T01:30:00Z"),
            ("2024-10-27T01:30:00Z", "2024-10-28T01:30:00Z"),
        ] {
            assert_eq!(
                next_alarm(time, &utc(now).with_timezone(&TestZone)).unwrap(),
                utc(expected)
            );
        }
    }
}
