//! ERG death spiral protection.
//!
//! Monitors cadence while in ERG mode and prevents the "death spiral" where
//! falling cadence causes increasing resistance, which further reduces cadence.

use std::time::{Duration, Instant};

/// Cadence below this triggers the death spiral timeout.
const LOW_CADENCE_THRESHOLD: f64 = 40.0;

/// Cadence must recover to this before ramping resumes.
const RECOVERY_CADENCE: f64 = 85.0;

/// How long cadence must stay below threshold before suspending ERG.
const LOW_CADENCE_TIMEOUT: Duration = Duration::from_secs(3);

/// Duration of the power ramp from 0 back to target.
const RAMP_DURATION: Duration = Duration::from_secs(15);

/// Internal state of the ERG death spiral protection.
#[derive(Debug)]
enum ErgState {
    /// No ERG target is active (non-ERG mode or no target set).
    Inactive,
    /// ERG target is active, cadence is normal.
    Normal,
    /// Cadence dropped below threshold; tracking how long.
    LowCadenceDetected { since: Instant },
    /// ERG suspended (power set to 0) waiting for cadence recovery.
    Suspended,
    /// Ramping power from 0 back to target over RAMP_DURATION.
    Ramping {
        started: Instant,
        low_since: Option<Instant>,
    },
}

/// Monitors cadence while in ERG mode and protects against death spirals.
///
/// When cadence drops below 40 RPM for 3 seconds, ERG power is set to 0.
/// When cadence recovers to 85 RPM, power ramps from 0 to the target over
/// 15 seconds. If cadence drops again during the ramp, power returns to 0.
#[derive(Debug)]
pub struct ErgSafetyMonitor {
    state: ErgState,
    target_watts: i16,
}

impl ErgSafetyMonitor {
    /// Create a new monitor in the Inactive state.
    pub fn new() -> Self {
        Self {
            state: ErgState::Inactive,
            target_watts: 0,
        }
    }

    /// Called when a SetTargetPower command is issued by the user.
    ///
    /// Updates the target and returns the actual power to send to the trainer.
    /// During suspension this returns 0; during ramping it returns an
    /// interpolated value.
    pub fn on_set_target_power(&mut self, watts: i16, now: Instant) -> i16 {
        self.target_watts = watts;
        match self.state {
            ErgState::Inactive => {
                self.state = ErgState::Normal;
                watts
            }
            ErgState::Normal | ErgState::LowCadenceDetected { .. } => watts,
            ErgState::Suspended => 0,
            ErgState::Ramping { started, .. } => self.ramp_power(started, now),
        }
    }

    /// Called when a non-ERG command is issued (resistance, simulation, etc.).
    ///
    /// Transitions back to Inactive — ERG safety is not relevant outside ERG mode.
    pub fn on_non_erg_command(&mut self) {
        self.state = ErgState::Inactive;
        self.target_watts = 0;
    }

    /// Called when a new cadence reading arrives from the trainer.
    ///
    /// Returns `Some(power)` if the monitor wants to override the current
    /// power sent to the trainer (e.g., switching to 0 on suspension or
    /// starting a ramp). Returns `None` if no override is needed.
    pub fn on_cadence_update(&mut self, cadence_rpm: Option<f64>, now: Instant) -> Option<i16> {
        // Treat missing cadence as 0 RPM for safety.
        let cadence = cadence_rpm.unwrap_or(0.0);

        match self.state {
            ErgState::Inactive => None,
            ErgState::Normal => {
                if cadence < LOW_CADENCE_THRESHOLD {
                    self.state = ErgState::LowCadenceDetected { since: now };
                }
                None
            }
            ErgState::LowCadenceDetected { since } => {
                if cadence >= LOW_CADENCE_THRESHOLD {
                    // False alarm — cadence recovered.
                    self.state = ErgState::Normal;
                    None
                } else if now.duration_since(since) >= LOW_CADENCE_TIMEOUT {
                    // Death spiral confirmed — suspend ERG.
                    self.state = ErgState::Suspended;
                    Some(0)
                } else {
                    None
                }
            }
            ErgState::Suspended => {
                if cadence >= RECOVERY_CADENCE {
                    // Cadence recovered — start ramping.
                    self.state = ErgState::Ramping {
                        started: now,
                        low_since: None,
                    };
                    Some(0) // Start ramp from 0
                } else {
                    None
                }
            }
            ErgState::Ramping {
                started,
                ref mut low_since,
            } => {
                if cadence < LOW_CADENCE_THRESHOLD {
                    match *low_since {
                        Some(ls) if now.duration_since(ls) >= LOW_CADENCE_TIMEOUT => {
                            // Dropped again during ramp — back to suspended.
                            self.state = ErgState::Suspended;
                            Some(0)
                        }
                        Some(_) => {
                            // Still counting down.
                            Some(self.ramp_power(started, now))
                        }
                        None => {
                            // Start tracking low cadence.
                            *low_since = Some(now);
                            Some(self.ramp_power(started, now))
                        }
                    }
                } else {
                    // Cadence is fine — clear any low tracking.
                    // Need to handle borrow checker by computing power first.
                    let power = self.ramp_power(started, now);
                    if let ErgState::Ramping {
                        ref mut low_since, ..
                    } = self.state
                    {
                        *low_since = None;
                    }
                    Some(power)
                }
            }
        }
    }

    /// Called periodically during the ramp phase (e.g., every 500ms).
    ///
    /// Returns the current ramp power, or `None` if not ramping.
    /// When the ramp completes, transitions to Normal and returns the full
    /// target power.
    pub fn on_ramp_tick(&mut self, now: Instant) -> Option<i16> {
        if let ErgState::Ramping { started, .. } = self.state {
            let power = self.ramp_power(started, now);
            if now.duration_since(started) >= RAMP_DURATION {
                self.state = ErgState::Normal;
            }
            Some(power)
        } else {
            None
        }
    }

    /// Returns `true` when the monitor is in a state that needs periodic ticks
    /// (i.e., during ramping).
    pub fn needs_tick(&self) -> bool {
        matches!(self.state, ErgState::Ramping { .. })
    }

    /// Compute the ramp power given the ramp start time and current time.
    fn ramp_power(&self, started: Instant, now: Instant) -> i16 {
        let elapsed = now.duration_since(started).as_secs_f64();
        let fraction = (elapsed / RAMP_DURATION.as_secs_f64()).min(1.0);
        (self.target_watts as f64 * fraction) as i16
    }
}

impl Default for ErgSafetyMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> Instant {
        Instant::now()
    }

    #[test]
    fn new_monitor_is_inactive() {
        let monitor = ErgSafetyMonitor::new();
        assert!(!monitor.needs_tick());
    }

    #[test]
    fn default_is_inactive() {
        let monitor = ErgSafetyMonitor::default();
        assert!(!monitor.needs_tick());
    }

    #[test]
    fn set_target_power_activates_normal() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        let power = monitor.on_set_target_power(200, t);
        assert_eq!(power, 200);
        assert!(!monitor.needs_tick());
    }

    #[test]
    fn cadence_update_in_inactive_returns_none() {
        let mut monitor = ErgSafetyMonitor::new();
        let result = monitor.on_cadence_update(Some(30.0), now());
        assert!(result.is_none());
    }

    #[test]
    fn normal_cadence_stays_normal() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        let result = monitor.on_cadence_update(Some(90.0), t);
        assert!(result.is_none());
    }

    #[test]
    fn low_cadence_triggers_detection() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);

        // Drop below threshold.
        let result = monitor.on_cadence_update(Some(35.0), t);
        assert!(result.is_none()); // Not yet suspended.
    }

    #[test]
    fn low_cadence_false_alarm_recovers() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);

        // Drop below threshold.
        monitor.on_cadence_update(Some(35.0), t);

        // Recover before timeout.
        let t2 = t + Duration::from_secs(2);
        let result = monitor.on_cadence_update(Some(50.0), t2);
        assert!(result.is_none()); // Back to Normal, no override.
    }

    #[test]
    fn low_cadence_for_3s_suspends_erg() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);

        // Drop below threshold.
        monitor.on_cadence_update(Some(35.0), t);

        // Wait 3 seconds with low cadence.
        let t2 = t + Duration::from_secs(3);
        let result = monitor.on_cadence_update(Some(30.0), t2);
        assert_eq!(result, Some(0)); // Suspended — power = 0.
    }

    #[test]
    fn suspended_ignores_low_cadence() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        monitor.on_cadence_update(Some(35.0), t);
        let t2 = t + Duration::from_secs(3);
        monitor.on_cadence_update(Some(30.0), t2);

        // Still low — no new override needed.
        let t3 = t2 + Duration::from_secs(1);
        let result = monitor.on_cadence_update(Some(20.0), t3);
        assert!(result.is_none());
    }

    #[test]
    fn suspended_partial_recovery_stays_suspended() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        monitor.on_cadence_update(Some(35.0), t);
        let t2 = t + Duration::from_secs(3);
        monitor.on_cadence_update(Some(30.0), t2);

        // Cadence recovers to 60 RPM — not enough (need 85).
        let t3 = t2 + Duration::from_secs(1);
        let result = monitor.on_cadence_update(Some(60.0), t3);
        assert!(result.is_none()); // Still suspended.
    }

    #[test]
    fn suspended_recovery_starts_ramp() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        monitor.on_cadence_update(Some(35.0), t);
        let t2 = t + Duration::from_secs(3);
        monitor.on_cadence_update(Some(30.0), t2);

        // Cadence recovers to 85 RPM.
        let t3 = t2 + Duration::from_secs(5);
        let result = monitor.on_cadence_update(Some(85.0), t3);
        assert_eq!(result, Some(0)); // Ramp starts from 0.
        assert!(monitor.needs_tick());
    }

    #[test]
    fn ramp_interpolates_power() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        monitor.on_cadence_update(Some(35.0), t);
        let t2 = t + Duration::from_secs(3);
        monitor.on_cadence_update(Some(30.0), t2);
        let t3 = t2 + Duration::from_secs(1);
        monitor.on_cadence_update(Some(85.0), t3); // Start ramp.

        // At 50% through the ramp (7.5s).
        let t4 = t3 + Duration::from_millis(7500);
        let result = monitor.on_ramp_tick(t4);
        assert_eq!(result, Some(100)); // 200 * 0.5 = 100
    }

    #[test]
    fn ramp_completes_to_full_power() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        monitor.on_cadence_update(Some(35.0), t);
        let t2 = t + Duration::from_secs(3);
        monitor.on_cadence_update(Some(30.0), t2);
        let t3 = t2 + Duration::from_secs(1);
        monitor.on_cadence_update(Some(85.0), t3);

        // At 15s — ramp complete.
        let t4 = t3 + Duration::from_secs(15);
        let result = monitor.on_ramp_tick(t4);
        assert_eq!(result, Some(200));
        assert!(!monitor.needs_tick()); // Back to Normal.
    }

    #[test]
    fn ramp_cadence_drop_re_suspends() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        monitor.on_cadence_update(Some(35.0), t);
        let t2 = t + Duration::from_secs(3);
        monitor.on_cadence_update(Some(30.0), t2);
        let t3 = t2 + Duration::from_secs(1);
        monitor.on_cadence_update(Some(85.0), t3); // Start ramp.

        // Cadence drops during ramp.
        let t4 = t3 + Duration::from_secs(2);
        monitor.on_cadence_update(Some(30.0), t4);

        // Still low for 3 more seconds.
        let t5 = t4 + Duration::from_secs(3);
        let result = monitor.on_cadence_update(Some(25.0), t5);
        assert_eq!(result, Some(0)); // Back to Suspended.
        assert!(!monitor.needs_tick());
    }

    #[test]
    fn ramp_cadence_dip_then_recovery() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        monitor.on_cadence_update(Some(35.0), t);
        let t2 = t + Duration::from_secs(3);
        monitor.on_cadence_update(Some(30.0), t2);
        let t3 = t2 + Duration::from_secs(1);
        monitor.on_cadence_update(Some(85.0), t3); // Start ramp.

        // Brief cadence dip during ramp.
        let t4 = t3 + Duration::from_secs(2);
        monitor.on_cadence_update(Some(30.0), t4);

        // Recover before 3s timeout.
        let t5 = t4 + Duration::from_secs(2);
        let result = monitor.on_cadence_update(Some(90.0), t5);
        assert!(result.is_some()); // Still ramping, returns ramp power.
        assert!(monitor.needs_tick());
    }

    #[test]
    fn non_erg_command_deactivates() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);

        monitor.on_non_erg_command();
        assert!(!monitor.needs_tick());

        // Cadence updates should be ignored.
        let result = monitor.on_cadence_update(Some(30.0), t);
        assert!(result.is_none());
    }

    #[test]
    fn target_update_during_suspension_returns_zero() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        monitor.on_cadence_update(Some(35.0), t);
        let t2 = t + Duration::from_secs(3);
        monitor.on_cadence_update(Some(30.0), t2);

        // User changes target while suspended.
        let t3 = t2 + Duration::from_secs(1);
        let power = monitor.on_set_target_power(300, t3);
        assert_eq!(power, 0); // Still suspended.
    }

    #[test]
    fn target_update_during_ramp_returns_interpolated() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        monitor.on_cadence_update(Some(35.0), t);
        let t2 = t + Duration::from_secs(3);
        monitor.on_cadence_update(Some(30.0), t2);
        let t3 = t2 + Duration::from_secs(1);
        monitor.on_cadence_update(Some(85.0), t3); // Start ramp.

        // At 50% through the ramp, change target to 300.
        let t4 = t3 + Duration::from_millis(7500);
        let power = monitor.on_set_target_power(300, t4);
        assert_eq!(power, 150); // 300 * 0.5 = 150
    }

    #[test]
    fn missing_cadence_treated_as_zero() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);

        // None cadence should be treated as 0 RPM (below threshold).
        monitor.on_cadence_update(None, t);

        // Wait 3s.
        let t2 = t + Duration::from_secs(3);
        let result = monitor.on_cadence_update(None, t2);
        assert_eq!(result, Some(0)); // Suspended.
    }

    #[test]
    fn on_ramp_tick_returns_none_when_not_ramping() {
        let mut monitor = ErgSafetyMonitor::new();
        let result = monitor.on_ramp_tick(now());
        assert!(result.is_none());

        // Also inactive after set_target_power.
        monitor.on_set_target_power(200, now());
        let result = monitor.on_ramp_tick(now());
        assert!(result.is_none());
    }

    #[test]
    fn full_lifecycle() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();

        // 1. Set target power.
        assert_eq!(monitor.on_set_target_power(200, t), 200);

        // 2. Normal cadence.
        assert!(monitor.on_cadence_update(Some(90.0), t).is_none());

        // 3. Cadence drops.
        let t1 = t + Duration::from_secs(1);
        assert!(monitor.on_cadence_update(Some(35.0), t1).is_none());

        // 4. 3s later — suspended.
        let t2 = t1 + Duration::from_secs(3);
        assert_eq!(monitor.on_cadence_update(Some(30.0), t2), Some(0));

        // 5. Cadence recovers to 85 — ramp starts.
        let t3 = t2 + Duration::from_secs(5);
        assert_eq!(monitor.on_cadence_update(Some(85.0), t3), Some(0));
        assert!(monitor.needs_tick());

        // 6. Ramp at 50%.
        let t4 = t3 + Duration::from_millis(7500);
        assert_eq!(monitor.on_ramp_tick(t4), Some(100));

        // 7. Ramp complete.
        let t5 = t3 + Duration::from_secs(15);
        assert_eq!(monitor.on_ramp_tick(t5), Some(200));
        assert!(!monitor.needs_tick());

        // 8. Back to normal — cadence updates don't override.
        assert!(monitor.on_cadence_update(Some(90.0), t5).is_none());
    }

    #[test]
    fn set_target_power_in_normal_returns_watts() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);

        // Update target while in normal state.
        let power = monitor.on_set_target_power(250, t);
        assert_eq!(power, 250);
    }

    #[test]
    fn set_target_power_in_low_cadence_detected_returns_watts() {
        let mut monitor = ErgSafetyMonitor::new();
        let t = now();
        monitor.on_set_target_power(200, t);
        monitor.on_cadence_update(Some(35.0), t); // LowCadenceDetected

        let power = monitor.on_set_target_power(250, t);
        assert_eq!(power, 250);
    }
}
