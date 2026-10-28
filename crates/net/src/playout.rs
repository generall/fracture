//! Jitter buffer: snapshots are presented on a smoothly advancing host clock,
//! delayed just enough to cover most measured arrival jitter. Timestamps are
//! the host's send times, so the countdown and host catch-up limits do not
//! look like network delay.
//!
//! Wi-Fi delivers most snapshots promptly but occasionally stalls, then
//! releases a burst. Covering every stall would add its whole length to all
//! motion for seconds afterwards. Instead the delay covers a high percentile
//! of recent lateness; rarer stalls are bridged by briefly extrapolating
//! motion, and the clock only ever changes speed gradually, never stepping
//! backwards.
use std::collections::VecDeque;

/// Seconds of arrival history for the latency baseline and jitter estimate.
const WINDOW: f64 = 3.0;
/// Share of recent snapshots the delay should cover without extrapolating.
const COVERAGE: f64 = 0.95;
const MARGIN: f64 = 0.005;
const MAX_DELAY: f64 = 0.3;
/// Motion continues this far past the newest snapshot, then holds.
pub const MAX_EXTRAPOLATION: f64 = 0.1;
/// A clock this far behind its target (first frames, host pause) jumps to it.
const RESYNC: f64 = 0.5;
/// Clock speed correction per second of error, and its limits. Slowing down
/// rebuilds the buffer after a stall; speeding up drains surplus delay.
const GAIN: f64 = 2.0;
const SLOWEST: f64 = 0.85;
const FASTEST: f64 = 1.15;

/// Presentation health, accumulated over the session.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Seconds presented beyond the newest snapshot, extrapolated or held.
    pub starved: f64,
    /// Seconds frozen at the extrapolation limit.
    pub held: f64,
    /// Forward clock jumps.
    pub resyncs: u32,
}

pub struct Playout<T> {
    queue: VecDeque<(f64, T)>,
    /// (local arrival time, arrival − host time) per snapshot.
    arrivals: VecDeque<(f64, f64)>,
    /// (local arrival time, host time since the previous snapshot).
    gaps: VecDeque<(f64, f64)>,
    newest: Option<f64>,
    clock: Option<f64>,
    last_local: f64,
    /// Host times of the previous and current presented frames.
    shown: (f64, f64),
    pub stats: Stats,
}
impl<T> Default for Playout<T> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            arrivals: VecDeque::new(),
            gaps: VecDeque::new(),
            newest: None,
            clock: None,
            last_local: 0.0,
            shown: (0.0, 0.0),
            stats: Stats::default(),
        }
    }
}
fn quantile(values: &mut [f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let k = ((values.len() - 1) as f64 * q).round() as usize;
    *values.select_nth_unstable_by(k, f64::total_cmp).1
}
impl<T> Playout<T> {
    /// `host` is the snapshot's send time and `now` its local arrival time,
    /// both in seconds on their own monotonic clocks.
    pub fn push(&mut self, host: f64, now: f64, value: T) {
        if self.newest.is_none_or(|newest| host > newest) {
            if let Some(newest) = self.newest {
                self.gaps.push_back((now, host - newest));
            }
            self.arrivals.push_back((now, now - host));
            self.newest = Some(host);
        }
        self.queue.push_back((host, value));
    }
    fn min_offset(&self) -> Option<f64> {
        self.arrivals.iter().map(|a| a.1).min_by(f64::total_cmp)
    }
    /// Typical snapshot spacing plus the covered share of recent lateness.
    pub fn delay(&self) -> f64 {
        let Some(min) = self.min_offset() else {
            return 0.0;
        };
        let mut late: Vec<_> = self.arrivals.iter().map(|a| a.1 - min).collect();
        let mut gaps: Vec<_> = self.gaps.iter().map(|g| g.1).collect();
        (quantile(&mut gaps, 0.5) + quantile(&mut late, COVERAGE) + MARGIN).min(MAX_DELAY)
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
        let (Some(min), Some(newest)) = (self.min_offset(), self.newest) else {
            return due;
        };
        // Without newer snapshots there is nothing to catch up to.
        let limit = newest + MAX_EXTRAPOLATION;
        let target = (now - min - self.delay()).min(limit);
        let clock = match self.clock {
            None => {
                // The first frame shows immediately; later ones follow the clock.
                if let Some((host, value)) = self.queue.pop_front() {
                    self.shown = (host, host);
                    due.push(value);
                }
                target
            }
            Some(clock) if target - clock > RESYNC => {
                self.stats.resyncs += 1;
                target
            }
            // A clock ahead of its target only slows down: it never steps back.
            Some(clock) => clock + dt * (1.0 + (target - clock) * GAIN).clamp(SLOWEST, FASTEST),
        };
        if clock > newest {
            self.stats.starved += dt;
        }
        if clock >= limit {
            self.stats.held += dt;
        }
        let clock = clock.min(limit);
        self.clock = Some(clock);
        while clock >= self.shown.1
            && let Some((host, value)) = self.queue.pop_front()
        {
            self.shown = (self.shown.1, host);
            due.push(value);
        }
        due
    }
    /// Position between the previous and current presented frames. Above 1
    /// while the newest snapshot is late: motion is extrapolated, bounded by
    /// MAX_EXTRAPOLATION.
    pub fn alpha(&self) -> f32 {
        let (from, to) = self.shown;
        match self.clock {
            Some(clock) if to > from => ((clock - from) / (to - from)).max(0.0) as f32,
            _ => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FPS: f64 = 60.0;
    const RATE: f64 = 30.0;
    /// Presents host-time "positions" at 60 fps; returns the presented
    /// position on every rendered frame after the first two seconds.
    fn present(arrival: impl Fn(usize) -> f64, seconds: f64) -> (Vec<(f64, f64)>, Playout<f64>) {
        let mut playout = Playout::default();
        let mut next = 0;
        let (mut from, mut to) = (0.0, 0.0);
        let mut presented = vec![];
        for frame in 0..(seconds * FPS) as usize {
            let now = frame as f64 / FPS;
            // In send order, as one TCP stream delivers them. Infinite: never sent.
            loop {
                let at = arrival(next);
                if at > now && at.is_finite() {
                    break;
                }
                if at.is_finite() {
                    playout.push(next as f64 / RATE, now, next as f64 / RATE);
                }
                next += 1;
            }
            for host in playout.advance(now) {
                (from, to) = (to, host);
            }
            if now >= 2.0 {
                presented.push((now, from + (to - from) * playout.alpha() as f64));
            }
        }
        (presented, playout)
    }
    fn steps(presented: &[(f64, f64)]) -> Vec<f64> {
        presented.windows(2).map(|w| w[1].1 - w[0].1).collect()
    }
    /// Deterministic pseudo-random value in [0, 1).
    fn noise(n: usize) -> f64 {
        ((n.wrapping_mul(2_654_435_761) >> 7) % 1000) as f64 / 1000.0
    }
    #[test]
    fn jittered_arrivals_present_smoothly_without_freezing() {
        let (presented, playout) = present(|n| n as f64 / RATE + 0.002 + noise(n) * 0.025, 10.0);
        let steps = steps(&presented);
        assert!(
            steps.iter().all(|&s| s >= -1e-9),
            "time never runs backwards"
        );
        let frozen = steps.iter().filter(|&&s| s < 1e-6).count();
        assert_eq!(frozen, 0, "no frames freeze while snapshots are buffered");
        let fastest = steps.iter().copied().fold(0.0, f64::max);
        assert!(fastest < 1.2 / FPS, "no visible jumps: {fastest}");
        assert!(playout.delay() < 0.08, "delay: {}", playout.delay());
        assert_eq!(playout.stats.resyncs, 0);
    }
    /// Wi-Fi: small jitter, and every ~1.7 s a 120–300 ms stall after which
    /// the held snapshots arrive together, in order, as TCP delivers them.
    fn wifi(n: usize) -> f64 {
        let send = n as f64 / RATE;
        let period = 1.7;
        let phase = send % period;
        let stall = 0.12 + noise(n / 51) * 0.18;
        let jitter = noise(n) * 0.008;
        if phase < stall {
            send - phase + stall + jitter
        } else {
            send + jitter
        }
    }
    #[test]
    fn wifi_stalls_neither_jump_nor_step_backwards_and_bound_lag() {
        let (presented, playout) = present(wifi, 30.0);
        let steps = steps(&presented);
        assert!(
            steps.iter().all(|&s| s >= -1e-9),
            "time never runs backwards"
        );
        let fastest = steps.iter().copied().fold(0.0, f64::max);
        assert!(
            fastest <= FASTEST / FPS + 1e-9,
            "no visible jumps: {fastest}"
        );
        assert_eq!(playout.stats.resyncs, 0);
        let lag = presented.iter().map(|(now, p)| now - p).sum::<f64>() / presented.len() as f64;
        assert!(
            lag < 0.17,
            "stalls must not inflate delay to their length: {lag}"
        );
        let frozen = steps.iter().filter(|&&s| s < 1e-6).count() as f64 / steps.len() as f64;
        assert!(frozen < 0.04, "frozen share: {frozen}");
    }
    #[test]
    fn delay_settles_once_the_network_recovers() {
        let (presented, playout) = present(
            |n| {
                if (n as f64) < 10.0 * RATE {
                    wifi(n)
                } else {
                    n as f64 / RATE + noise(n) * 0.004
                }
            },
            20.0,
        );
        assert!(playout.delay() < 0.05, "delay: {}", playout.delay());
        let (now, position) = presented.last().unwrap();
        assert!(now - position < 0.06, "lag: {}", now - position);
        assert!(steps(&presented).iter().all(|&s| s >= -1e-9));
    }
    #[test]
    fn host_pause_resynchronizes_forward_once() {
        // The host stops sending for 1.5 s; send times continue afterwards.
        let (presented, playout) = present(
            |n| {
                let send = n as f64 / RATE;
                if (5.0..6.5).contains(&send) {
                    f64::INFINITY
                } else {
                    send + 0.003
                }
            },
            10.0,
        );
        assert_eq!(playout.stats.resyncs, 1);
        assert!(steps(&presented).iter().all(|&s| s >= -1e-9));
        let held = playout.stats.held;
        assert!(held > 1.0 && held < 1.6, "held: {held}");
    }
}
