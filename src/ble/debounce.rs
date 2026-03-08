// libsmarttrainer - Library for controlling a bicycle smart trainer
// Copyright (C) 2026 Kris Hardy <hardyrk@gmail.com>
//
// This library is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 2.1 of the License, or (at your option) any later version.
//
// This library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public
// License along with this library; if not, write to the Free Software
// Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301
// USA

use std::time::{Duration, Instant};

use crate::ble::commands::TrainerCommand;

/// Default minimum interval between commands sent to the trainer.
const DEFAULT_MIN_INTERVAL: Duration = Duration::from_secs(1);

/// Debounces control commands sent to the trainer.
///
/// Some trainers reject rapid command sequences. This struct enforces a minimum
/// interval between writes. When a command arrives too soon, it is stored as
/// pending (latest-wins). The caller should poll for the pending command after
/// the interval elapses.
///
/// Safety-critical commands (ERG safety overrides) bypass the debouncer
/// entirely, but should call [`CommandDebouncer::record_write`] afterward to
/// reset the timer.
pub struct CommandDebouncer {
    last_write: Option<Instant>,
    pending: Option<TrainerCommand>,
    min_interval: Duration,
}

impl Default for CommandDebouncer {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandDebouncer {
    /// Create a new debouncer with the default 1-second interval.
    pub fn new() -> Self {
        Self {
            last_write: None,
            pending: None,
            min_interval: DEFAULT_MIN_INTERVAL,
        }
    }

    /// Create a new debouncer with a custom interval.
    pub fn with_interval(min_interval: Duration) -> Self {
        Self {
            last_write: None,
            pending: None,
            min_interval,
        }
    }

    /// Submit a command. Returns `Some(cmd)` if enough time has elapsed since
    /// the last write and the command can be sent immediately. Otherwise stores
    /// the command as pending (overwrites any existing pending command) and
    /// returns `None`.
    pub fn submit(&mut self, cmd: TrainerCommand, now: Instant) -> Option<TrainerCommand> {
        if self.can_send(now) {
            self.last_write = Some(now);
            self.pending = None;
            Some(cmd)
        } else {
            self.pending = Some(cmd);
            None
        }
    }

    /// Poll for a pending command. Returns `Some(cmd)` if there is a pending
    /// command and enough time has elapsed since the last write.
    pub fn poll_pending(&mut self, now: Instant) -> Option<TrainerCommand> {
        if self.pending.is_some() && self.can_send(now) {
            self.last_write = Some(now);
            self.pending.take()
        } else {
            None
        }
    }

    /// Returns the remaining wait time before a pending command can be sent.
    /// Returns `None` if there is no pending command.
    pub fn time_until_next(&self, now: Instant) -> Option<Duration> {
        self.pending.as_ref()?;
        match self.last_write {
            Some(last) => {
                let elapsed = now.duration_since(last);
                if elapsed >= self.min_interval {
                    Some(Duration::ZERO)
                } else {
                    Some(self.min_interval - elapsed)
                }
            }
            None => Some(Duration::ZERO),
        }
    }

    /// Record that a write was performed externally (e.g., by an ERG safety
    /// override). This resets the timer so the next user command respects the
    /// interval.
    pub fn record_write(&mut self, now: Instant) {
        self.last_write = Some(now);
    }

    fn can_send(&self, now: Instant) -> bool {
        match self.last_write {
            Some(last) => now.duration_since(last) >= self.min_interval,
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd_power(watts: i16) -> TrainerCommand {
        TrainerCommand::SetTargetPower(watts)
    }

    fn cmd_resistance(level: u8) -> TrainerCommand {
        TrainerCommand::SetTargetResistance(level)
    }

    #[test]
    fn first_command_sends_immediately() {
        let mut d = CommandDebouncer::new();
        let now = Instant::now();
        assert_eq!(d.submit(cmd_power(200), now), Some(cmd_power(200)));
    }

    #[test]
    fn command_within_interval_is_debounced() {
        let mut d = CommandDebouncer::new();
        let t0 = Instant::now();
        d.submit(cmd_power(200), t0);

        let t1 = t0 + Duration::from_millis(500);
        assert_eq!(d.submit(cmd_power(250), t1), None);
    }

    #[test]
    fn command_after_interval_sends_immediately() {
        let mut d = CommandDebouncer::new();
        let t0 = Instant::now();
        d.submit(cmd_power(200), t0);

        let t1 = t0 + Duration::from_secs(1);
        assert_eq!(d.submit(cmd_power(250), t1), Some(cmd_power(250)));
    }

    #[test]
    fn poll_pending_returns_command_after_interval() {
        let mut d = CommandDebouncer::new();
        let t0 = Instant::now();
        d.submit(cmd_power(200), t0);

        let t1 = t0 + Duration::from_millis(500);
        d.submit(cmd_power(250), t1);

        let t2 = t0 + Duration::from_secs(1);
        assert_eq!(d.poll_pending(t2), Some(cmd_power(250)));
    }

    #[test]
    fn poll_pending_returns_none_without_pending() {
        let mut d = CommandDebouncer::new();
        let now = Instant::now();
        assert_eq!(d.poll_pending(now), None);
    }

    #[test]
    fn rapid_commands_only_latest_is_kept() {
        let mut d = CommandDebouncer::new();
        let t0 = Instant::now();
        d.submit(cmd_power(200), t0);

        let t1 = t0 + Duration::from_millis(100);
        d.submit(cmd_power(250), t1);

        let t2 = t0 + Duration::from_millis(200);
        d.submit(cmd_resistance(50), t2);

        let t3 = t0 + Duration::from_secs(1);
        assert_eq!(d.poll_pending(t3), Some(cmd_resistance(50)));
    }

    #[test]
    fn time_until_next_returns_remaining_duration() {
        let mut d = CommandDebouncer::new();
        let t0 = Instant::now();
        d.submit(cmd_power(200), t0);

        let t1 = t0 + Duration::from_millis(300);
        d.submit(cmd_power(250), t1);

        let remaining = d.time_until_next(t1).unwrap();
        assert!(remaining >= Duration::from_millis(690));
        assert!(remaining <= Duration::from_millis(710));
    }

    #[test]
    fn time_until_next_returns_none_without_pending() {
        let d = CommandDebouncer::new();
        let now = Instant::now();
        assert_eq!(d.time_until_next(now), None);
    }

    #[test]
    fn custom_interval_works() {
        let mut d = CommandDebouncer::with_interval(Duration::from_millis(200));
        let t0 = Instant::now();
        d.submit(cmd_power(200), t0);

        let t1 = t0 + Duration::from_millis(100);
        assert_eq!(d.submit(cmd_power(250), t1), None);

        let t2 = t0 + Duration::from_millis(200);
        assert_eq!(d.submit(cmd_power(300), t2), Some(cmd_power(300)));
    }

    #[test]
    fn record_write_resets_timer() {
        let mut d = CommandDebouncer::new();
        let t0 = Instant::now();
        d.submit(cmd_power(200), t0);

        // Safety override happens at t0 + 800ms.
        let t1 = t0 + Duration::from_millis(800);
        d.record_write(t1);

        // User command at t0 + 1000ms — only 200ms since safety write, should debounce.
        let t2 = t0 + Duration::from_millis(1000);
        assert_eq!(d.submit(cmd_power(300), t2), None);

        // After full interval from safety write, should send.
        let t3 = t1 + Duration::from_secs(1);
        assert_eq!(d.poll_pending(t3), Some(cmd_power(300)));
    }
}
