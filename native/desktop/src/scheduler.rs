//! One process-wide heavy-stage lease. Unconfirmed cleanup fails closed.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
struct GateState {
    busy: AtomicBool,
    poisoned: AtomicBool,
}

#[derive(Clone, Default)]
pub struct ResourceGate {
    state: Arc<GateState>,
}

pub struct StageLease {
    state: Arc<GateState>,
    cleanup_required: bool,
}
pub fn global() -> &'static ResourceGate {
    static GATE: std::sync::OnceLock<ResourceGate> = std::sync::OnceLock::new();
    GATE.get_or_init(Default::default)
}

impl ResourceGate {
    pub fn acquire(&self) -> Result<StageLease, String> {
        if self.state.poisoned.load(Ordering::Acquire) {
            return Err("Worker cleanup was not confirmed. Restart the native application before launching more inference.".into());
        }
        self.state.busy.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire)
            .map_err(|_|"Another heavy stage owns the resource budget. Wait or cancel it before starting another.".to_string())?;
        // Recheck after acquiring: a retiring lease may have poisoned the gate.
        if self.state.poisoned.load(Ordering::Acquire) {
            self.state.busy.store(false, Ordering::Release);
            return Err("Worker cleanup is unresolved; replacement work is blocked.".into());
        }
        Ok(StageLease {
            state: self.state.clone(),
            cleanup_required: false,
        })
    }
}

impl StageLease {
    pub fn require_cleanup(&mut self) {
        self.cleanup_required = true;
    }
    pub fn confirm_cleanup(&mut self) {
        self.cleanup_required = false;
    }
}

impl Drop for StageLease {
    fn drop(&mut self) {
        if self.cleanup_required {
            self.state.poisoned.store(true, Ordering::Release);
        }
        self.state.busy.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn releases_after_rejection_but_blocks_unconfirmed_cleanup() {
        let gate = ResourceGate::default();
        {
            let _lease = gate.acquire().unwrap();
            assert!(gate.acquire().is_err());
        }
        {
            let mut lease = gate.acquire().unwrap();
            lease.require_cleanup();
            lease.confirm_cleanup();
        }
        {
            let mut lease = gate.acquire().unwrap();
            lease.require_cleanup();
        }
        assert!(gate.acquire().err().unwrap().contains("cleanup"));
    }
    #[test]
    fn concurrent_requests_cannot_share_a_budget() {
        use std::sync::{Barrier, atomic::AtomicUsize};
        let gate = ResourceGate::default();
        let start = Arc::new(Barrier::new(8));
        let admitted = Arc::new(Barrier::new(8));
        let count = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let (gate, start, admitted, count) =
                    (gate.clone(), start.clone(), admitted.clone(), count.clone());
                std::thread::spawn(move || {
                    start.wait();
                    let lease = gate.acquire();
                    if lease.is_ok() {
                        count.fetch_add(1, Ordering::SeqCst);
                    }
                    admitted.wait();
                    drop(lease);
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(gate.acquire().is_ok());
    }
}
