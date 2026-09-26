//! Jitter buffer: snapshots are presented on a smoothly advancing host clock,
//! delayed just enough to cover measured arrival jitter, instead of the moment
//! each one happens to arrive.
use std::collections::VecDeque;

const WINDOW: f64 = 3.0;
const MAX_DELAY: f64 = 0.3;
/// Offsets beyond any plausible jitter mean the host clock paused (countdown,
/// capped catch-up); resynchronize instead of treating it as network delay.
const DISCONTINUITY: f64 = 0.5;

pub struct Playout<T> {
    queue: VecDeque<(f64, T)>,
    /// (local arrival time, arrival - host time) for frames that advanced the host clock.
    arrivals: VecDeque<(f64, f64)>,
    /// (local arrival time, host time between consecutive snapshots).
    gaps: VecDeque<(f64, f64)>,
    last_host: Option<f64>,
    clock: Option<f64>,
    last_local: f64,
    /// Host times of the previous and current presented frames.
    shown: (f64, f64),
}
impl<T> Default for Playout<T> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            arrivals: VecDeque::new(),
            gaps: VecDeque::new(),
            last_host: None,
            clock: None,
            last_local: 0.0,
            shown: (0.0, 0.0),
        }
    }
}
impl<T> Playout<T> {
    pub fn push(&mut self, host: f64, now: f64, value: T) {
        if self.last_host.is_none_or(|last| host > last) {
            let offset = now - host;
            if self
                .min_offset()
                .is_some_and(|min| offset - min > DISCONTINUITY)
            {
                self.arrivals.clear();
                self.gaps.clear();
                self.clock = None;
            }
            if let Some(last) = self.last_host {
                self.gaps.push_back((now, host - last));
            }
            self.arrivals.push_back((now, offset));
            self.last_host = Some(host);
        }
        self.queue.push_back((host, value));
    }
    fn min_offset(&self) -> Option<f64> {
        self.arrivals.iter().map(|a| a.1).min_by(f64::total_cmp)
    }
    /// Snapshot spacing plus the worst recent lateness, bounded.
    pub fn delay(&self) -> f64 {
        let Some(min) = self.min_offset() else {
            return 0.0;
        };
        let jitter = self.arrivals.iter().map(|a| a.1 - min).fold(0.0, f64::max);
        let interval = self.gaps.iter().map(|g| g.1).fold(0.0, f64::max);
        (interval + jitter + 0.005).min(MAX_DELAY)
    }
    /// Advances the presentation clock and returns frames that became due, in order.
    pub fn advance(&mut self, now: f64) -> Vec<T> {
        while self.arrivals.len() > 1 && self.arrivals[0].0 < now - WINDOW {
            self.arrivals.pop_front();
        }
        while self.gaps.len() > 1 && self.gaps[0].0 < now - WINDOW {
            self.gaps.pop_front();
        }
        let dt = (now - self.last_local).max(0.0);
        self.last_local = now;
        let mut due = vec![];
        let Some(min) = self.min_offset() else {
            return due;
        };
        let target = now - min - self.delay();
        let clock = match self.clock {
            None => {
                // The first frame shows immediately; later ones follow the clock.
                if let Some((host, value)) = self.queue.pop_front() {
                    self.shown = (host, host);
                    due.push(value);
                }
                target
            }
            // Slew gently towards the target; snap after large disruptions.
            Some(clock) if (target - clock).abs() > 0.25 => target,
            Some(clock) => clock + dt + ((target - clock) * 2.0 * dt).clamp(-0.2 * dt, 0.2 * dt),
        };
        self.clock = Some(clock);
        while clock >= self.shown.1
            && let Some((host, value)) = self.queue.pop_front()
        {
            self.shown = (self.shown.1, host);
            due.push(value);
        }
        due
    }
    /// Interpolation factor between the previous and current presented frames.
    pub fn alpha(&self) -> f32 {
        let (from, to) = self.shown;
        match self.clock {
            Some(clock) if to > from => ((clock - from) / (to - from)).clamp(0.0, 1.0) as f32,
            _ => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 30 Hz host snapshots with up to 25 ms of arrival jitter, presented at 60 fps.
    #[test]
    fn jittered_arrivals_present_smoothly_without_freezing() {
        let mut playout = Playout::default();
        let mut next = 0;
        let mut positions = vec![];
        let mut shown = 0.0;
        for frame in 0..60 * 10 {
            let now = frame as f64 / 60.0;
            // Pseudo-random lateness, deterministic.
            while {
                let host = next as f64 / 30.0;
                let late = ((next * 7919) % 26) as f64 / 1000.0;
                host + 0.002 + late <= now
            } {
                playout.push(next as f64 / 30.0, now, next as f64 / 30.0);
                next += 1;
            }
            for host in playout.advance(now) {
                shown = host;
            }
            let previous = (shown - 1.0 / 30.0f64).max(0.0);
            positions.push(previous + (shown - previous) * playout.alpha() as f64);
        }
        let steps: Vec<f64> = positions[120..].windows(2).map(|w| w[1] - w[0]).collect();
        assert!(
            steps.iter().all(|&s| s >= -1e-9),
            "time never runs backwards"
        );
        let frozen = steps.iter().filter(|&&s| s < 1e-6).count();
        assert_eq!(frozen, 0, "no frames freeze while snapshots are buffered");
        let fastest = steps.iter().copied().fold(0.0, f64::max);
        assert!(fastest < 2.5 / 60.0, "no visible jumps: {fastest}");
        assert!(
            playout.delay() < 0.1,
            "delay stays small: {}",
            playout.delay()
        );
    }
    #[test]
    fn countdown_pause_resynchronizes_instead_of_inflating_delay() {
        let mut playout = Playout::default();
        for i in 0..60 {
            playout.push(0.0, i as f64 / 20.0, ());
            playout.advance(i as f64 / 20.0);
        }
        for i in 0..30 {
            let now = 3.0 + i as f64 / 30.0;
            playout.push(i as f64 / 30.0, now, ());
            playout.advance(now);
        }
        assert!(playout.delay() < 0.05, "{}", playout.delay());
    }
}
