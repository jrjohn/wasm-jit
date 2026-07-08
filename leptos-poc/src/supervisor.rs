//! supervisor.rs — per-cell supervision (docs §18, scaling layer #3).
//!
//! A trap (fuel exhaustion, out-of-bounds slot, bad math) must not take the
//! app down or wedge the cell: the supervisor catches the error, serves the
//! last good value, rebuilds the instance (cheap — the module cache makes it
//! near-free), and quarantines a repeat offender after MAX_FAILURES so a
//! permanently-broken seed stops consuming rebuilds. `restart()` gives it a
//! fresh chance (e.g. after the user edits the script).

use crate::cell::Cell;

pub const MAX_FAILURES: u32 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Health {
    Healthy,
    /// At least one recent trap; serving last-good values between rebuilds.
    Degraded,
    /// MAX_FAILURES consecutive traps; no longer rebuilt until restart().
    Quarantined,
}

impl Health {
    pub fn label(&self) -> &'static str {
        match self {
            Health::Healthy => "healthy",
            Health::Degraded => "degraded",
            Health::Quarantined => "quarantined",
        }
    }
}

pub struct Supervised {
    build: Box<dyn Fn() -> Result<Cell, String>>,
    cell: Option<Cell>,
    pub last_good: f64,
    pub failures: u32,
    pub last_error: String,
}

impl Supervised {
    /// Build the cell now; a build error starts it quarantined (with the error).
    pub fn new(build: impl Fn() -> Result<Cell, String> + 'static) -> Self {
        let build: Box<dyn Fn() -> Result<Cell, String>> = Box::new(build);
        let (cell, failures, last_error) = match build() {
            Ok(c) => (Some(c), 0, String::new()),
            Err(e) => (None, MAX_FAILURES, e),
        };
        Supervised { build, cell, last_good: 0.0, failures, last_error }
    }

    pub fn health(&self) -> Health {
        if self.failures >= MAX_FAILURES {
            Health::Quarantined
        } else if self.failures > 0 {
            Health::Degraded
        } else {
            Health::Healthy
        }
    }

    /// Run the cell; on a trap, record it, serve the last good value, and
    /// rebuild (consecutive failures escalate to quarantine). A successful
    /// call heals the failure count back to zero.
    pub fn call(&mut self, args: &[f64]) -> f64 {
        let Some(cell) = self.cell.as_ref() else {
            return self.last_good; // quarantined (or never built)
        };
        match cell.call(args) {
            Ok(v) => {
                self.last_good = v;
                self.failures = 0;
                self.last_error.clear();
                v
            }
            Err(e) => {
                self.failures += 1;
                self.last_error = e;
                if self.failures >= MAX_FAILURES {
                    self.cell = None; // stop paying for rebuilds
                } else {
                    // A trapped instance may hold inconsistent internal state;
                    // rebuild from the (cached) module.
                    self.cell = (self.build)().ok();
                }
                self.last_good
            }
        }
    }

    /// Human decision (or a new script): rebuild and forgive.
    pub fn restart(&mut self) {
        match (self.build)() {
            Ok(c) => {
                self.cell = Some(c);
                self.failures = 0;
                self.last_error.clear();
            }
            Err(e) => {
                self.cell = None;
                self.failures = MAX_FAILURES;
                self.last_error = e;
            }
        }
    }

    /// Access the live cell (e.g. for write_mem/read_mem/fuel_used).
    pub fn cell(&self) -> Option<&Cell> {
        self.cell.as_ref()
    }
}
