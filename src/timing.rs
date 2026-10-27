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
