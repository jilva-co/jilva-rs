//! Experiment 04 — `Send` policy for the single-threaded component loop.
//!
//! Hypothesis H4: a universal `Send` bound on models/slots excludes idiomatic
//! single-threaded interiors (`Rc`, `RefCell`) while buying nothing for a
//! loop with no executor. `Send` belongs ONLY at the async-bridge boundary —
//! on completion values crossing threads — leaving the core loop unbounded.
//!
//! Addresses RFC-0002 open question 3 and ARCHITECTURE_FEASIBILITY R2-C4.
//! No Tokio, no executor, std `thread` only (and only in the bridge test).

#![forbid(unsafe_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Plain model: `Send`, no interiors.
pub struct PlainModel {
    /// Counter value.
    pub count: i64,
}

/// Single-threaded model using `Rc` + `RefCell` interiors (memoised derived
/// cache + shared list handle — the §3 "observably pure memo" shape).
pub struct SharedModel {
    /// Shared row cache.
    pub rows: Rc<RefCell<Vec<String>>>,
    /// Width-computed cursor (interior for render-helper convenience).
    pub cursor: Cell<usize>,
}

/// Framework-owned component-local slot with single-threaded interiors.
pub struct Slot {
    /// Draft text.
    pub draft: String,
    /// Memoised visible-window slice (observably pure cache).
    pub memo: Rc<RefCell<Vec<String>>>,
}

/// Compile-time capability probe (positive direction only; the negative
/// direction lives in `src/negative_send_bound.rs`).
pub fn assert_send<T: Send>() {}

/// The synchronous core loop: deliberately UNBOUNDED. `update` takes `&mut`,
/// `view` takes `&`; neither requires `Send` nor `Sync` because neither
/// crosses threads.
pub fn run_update<S, M>(slot: &mut S, msg: M, f: impl FnOnce(&mut S, M)) {
    f(slot, msg);
}

/// Data-only effects. `After` is pure data. `Task` is an OPAQUE closure:
/// the core loop never executes it, never names a thread or executor — it
/// only routes the value back to the caller, which hands it to the bridge.
/// (Answers R2-C9 executor-by-stealth: execution policy lives OUTSIDE core.)
pub enum Effect<M> {
    /// No effect.
    None,
    /// Timer data: deliver `M` after `millis`.
    After(u64, M),
    /// Opaque background job producing a completion message. `Send` bound
    /// here is the BRIDGE's requirement surfacing at the type level — not a
    /// bound on models or slots.
    Task(Box<dyn FnOnce() -> M + Send>),
}

/// Opt-in bridge ABOVE core. The ONLY `Send` bounds in the design live here:
/// the job must travel to a worker (`F: Send`) and the completion must travel
/// back (`Out: Send`). Slot/model state never crosses.
pub struct Bridge;

impl Bridge {
    /// Execute `job` on a worker thread (stand-in for any executor).
    pub fn spawn_job<F, Out>(job: F) -> std::thread::JoinHandle<Out>
    where
        F: FnOnce() -> Out + Send + 'static,
        Out: Send + 'static,
    {
        std::thread::spawn(job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_threaded_interiors_work_in_model_and_slot() {
        let m = SharedModel {
            rows: Rc::new(RefCell::new(vec!["a".into()])),
            cursor: Cell::new(0),
        };
        m.rows.borrow_mut().push("b".into());
        m.cursor.set(m.rows.borrow().len());
        assert_eq!(m.cursor.get(), 2);

        let mut s = Slot {
            draft: String::from("hi"),
            memo: Rc::new(RefCell::new(Vec::new())),
        };
        run_update(&mut s, "!", |slot, suffix| {
            slot.draft.push_str(suffix);
            slot.memo.borrow_mut().push(slot.draft.clone());
        });
        assert_eq!(s.draft, "hi!");
        assert_eq!(s.memo.borrow().as_slice(), ["hi!"]);
    }

    #[test]
    fn plain_state_is_send() {
        assert_send::<PlainModel>();
        assert_send::<CounterSlot>();
        // `Cell<usize>` is Send (only !Sync): even cursor cells may cross if
        // a bridge ever wants them — no policy decision needed either way.
        assert_send::<Cell<usize>>();
    }

    #[test]
    fn unbounded_core_accepts_not_send_state() {
        // If the loop required `S: Send`, this function could not exist.
        // It compiles and runs: the core is agnostic to thread-transfer.
        let m = SharedModel {
            rows: Rc::new(RefCell::new(Vec::new())),
            cursor: Cell::new(0),
        };
        let mut slot = m;
        run_update(&mut slot, "x".to_string(), |s, row| {
            s.rows.borrow_mut().push(row);
        });
        assert_eq!(slot.rows.borrow().len(), 1);
    }

    #[test]
    fn send_lives_only_at_bridge_boundary() {
        // Worker produces a completion; ONLY the completion crosses threads.
        // The !Send model and slot never leave this thread.
        let handle = Bridge::spawn_job(|| Completion { rows: vec!["r1".into(), "r2".into()] });
        let done: Completion = handle.join().expect("worker finished");

        let mut model = PlainModel { count: 0 };
        run_update(&mut model, done, |m, c| {
            m.count = c.rows.len() as i64;
        });
        assert_eq!(model.count, 2);
    }

    #[test]
    fn opaque_task_executed_externally_never_by_core() {
        // Core routes the effect value without executing or naming threads.
        let eff: Effect<Completion> = Effect::Task(Box::new(|| Completion {
            rows: vec!["bg".into()],
        }));
        let routed: Effect<Completion> = route_without_executing(eff);
        let completion = match routed {
            Effect::Task(job) => Bridge::spawn_job(job).join().expect("done"),
            _ => panic!("task must survive routing unexecuted"),
        };
        assert_eq!(completion.rows, ["bg"]);
        let timer: Effect<Completion> = Effect::After(100, Completion { rows: Vec::new() });
        assert!(matches!(timer, Effect::After(100, _)));
        let _ = Effect::<Completion>::None;
    }

    /// Stand-in for one core-loop routing step: moves the effect value, runs
    /// nothing, spawns nothing. If core ever needed an executor, this
    /// function could not stay this shape.
    fn route_without_executing<M>(eff: Effect<M>) -> Effect<M> {
        eff
    }

    /// Bridge completion: small owned data, `Send` by construction.
    struct Completion {
        rows: Vec<String>,
    }

    /// Plain counter slot: `Send` without trying.
    struct CounterSlot {
        #[allow(dead_code)]
        n: i64,
    }
}
