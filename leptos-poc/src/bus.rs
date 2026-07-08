//! bus.rs — the host-side event bus (docs §18, scaling layer #5).
//!
//! Cells never call each other: a cell's output enters the bus, and the bus
//! dispatches along subscription wires (themselves schema data) to downstream
//! cells — breadth-first, with a hard dispatch budget so a wiring cycle or a
//! fan-out storm degrades into a reported overflow instead of a hang. This is
//! §8's synapse, landed: the topology lives in data, the host owns delivery.

use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub struct Wire {
    pub from: String,
    pub to: String,
}

pub struct DispatchReport {
    /// Number of downstream cell runs performed (excludes the origin's own run).
    pub dispatches: u32,
    /// True when the budget cut the cascade short (cycle or storm).
    pub overflow: bool,
}

pub const DEFAULT_BUDGET: u32 = 64;

/// Propagate `value` from `origin` along `wires`. `run` executes one cell with
/// one argument and returns its output (None = cell unavailable/quarantined —
/// the cascade stops on that branch). Budget counts cell runs, not edges.
pub fn dispatch(
    wires: &[Wire],
    origin: &str,
    value: f64,
    budget: u32,
    mut run: impl FnMut(&str, f64) -> Option<f64>,
) -> DispatchReport {
    let mut queue: VecDeque<(String, f64)> = VecDeque::new();
    for w in wires.iter().filter(|w| w.from == origin) {
        queue.push_back((w.to.clone(), value));
    }
    let mut dispatches = 0u32;
    while let Some((id, v)) = queue.pop_front() {
        if dispatches >= budget {
            return DispatchReport { dispatches, overflow: true };
        }
        dispatches += 1;
        let Some(out) = run(&id, v) else { continue };
        for w in wires.iter().filter(|w| w.from == id) {
            queue.push_back((w.to.clone(), out));
        }
    }
    DispatchReport { dispatches, overflow: false }
}
