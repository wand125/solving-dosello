//! Interior YBWC. A bounded OS worker pool steals younger siblings from live
//! split points. Masters work while waiting; no OS thread is created per split.
use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub(super) struct Cancellation {
    cut: AtomicBool,
    parent: Option<Arc<Cancellation>>,
}
impl Cancellation {
    fn cancelled(&self) -> bool {
        self.cut.load(Relaxed) || self.parent.as_ref().is_some_and(|p| p.cancelled())
    }
}
struct State {
    next: usize,
    active: usize,
    alpha: i32,
    best: i32,
    best_id: u16,
    failed: bool,
}
struct Split {
    p: Position,
    key: u64,
    depth: u8,
    beta: i32,
    moves: Vec<Move>,
    state: Mutex<State>,
    cancel: Arc<Cancellation>,
}
#[repr(align(128))]
struct Counter(AtomicUsize);
pub(super) struct Pool {
    pub(super) min_depth: u8,
    width: usize,
    queue: Mutex<VecDeque<Arc<Split>>>,
    wake: Condvar,
    idle: Counter,
    stop: AtomicBool,
    splits: Counter,
    stolen: Counter,
}
impl Pool {
    pub(super) fn has_idle(&self) -> bool {
        self.idle.0.load(Relaxed) > 0
    }
    fn take(&self, ancestor: Option<&Arc<Cancellation>>) -> Option<(Arc<Split>, Move, i32)> {
        let mut queue = self.queue.lock().unwrap();
        let count = queue.len();
        for _ in 0..count {
            let split = queue.pop_front()?;
            if let Some(ancestor) = ancestor {
                let mut token = Some(&split.cancel);
                while let Some(c) = token {
                    if Arc::ptr_eq(c, ancestor) {
                        break;
                    }
                    token = c.parent.as_ref();
                }
                if token.is_none() {
                    queue.push_back(split);
                    continue;
                }
            }
            let mut state = split.state.lock().unwrap();
            if split.cancel.cancelled() || state.next == split.moves.len() {
                continue;
            }
            if state.active >= self.width {
                drop(state);
                queue.push_back(split);
                continue;
            }
            let mv = split.moves[state.next];
            state.next += 1;
            state.active += 1;
            let alpha = state.alpha;
            if state.next < split.moves.len() {
                queue.push_back(split.clone());
            }
            drop(state);
            self.stolen.0.fetch_add(1, Relaxed);
            return Some((split, mv, alpha));
        }
        None
    }
}
impl Search {
    pub(super) fn cancelled(&self) -> bool {
        self.cancellation.as_ref().is_some_and(|c| c.cancelled())
    }
    fn work(&mut self, split: Arc<Split>, mv: Move, alpha: i32) {
        let previous = self.cancellation.replace(split.cancel.clone());
        let child = split.p.play(mv);
        let key = split.p.child_hash(split.key, mv);
        let result = (|| {
            if self.cancelled() {
                return Err(());
            }
            let mut v = -self.run_key(child, key, split.depth - 1, -alpha - 1, -alpha)?;
            if v > alpha && v < split.beta {
                // A full-window confirmation is needed before raising alpha.
                v = -self.run_key(child, key, split.depth - 1, -split.beta, -alpha)?;
            }
            Ok(v)
        })();
        self.cancellation = previous;
        let mut state = split.state.lock().unwrap();
        state.active -= 1;
        match result {
            Ok(v) => {
                if v > state.best {
                    state.best = v;
                    state.best_id = mv.id();
                }
                state.alpha = state.alpha.max(v);
                if state.alpha >= split.beta {
                    split.cancel.cut.store(true, Relaxed);
                }
            }
            Err(()) => {
                if !split.cancel.cancelled() {
                    state.failed = true;
                    split.cancel.cut.store(true, Relaxed);
                }
            }
        }
        if let Some(pool) = &self.parallel {
            pool.wake.notify_all();
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn split(
        &mut self,
        pool: Arc<Pool>,
        p: Position,
        key: u64,
        depth: u8,
        alpha: i32,
        beta: i32,
        best: i32,
        best_id: u16,
        moves: Vec<Move>,
    ) -> Result<(i32, u16), ()> {
        let split = Arc::new(Split {
            p,
            key,
            depth,
            beta,
            moves,
            state: Mutex::new(State {
                next: 0,
                active: 0,
                alpha,
                best,
                best_id,
                failed: false,
            }),
            cancel: Arc::new(Cancellation {
                cut: AtomicBool::new(false),
                parent: self.cancellation.clone(),
            }),
        });
        pool.splits.0.fetch_add(1, Relaxed);
        pool.queue.lock().unwrap().push_front(split.clone());
        pool.wake.notify_all();
        loop {
            {
                let state = split.state.lock().unwrap();
                if state.active == 0
                    && (state.next == split.moves.len() || split.cancel.cancelled())
                {
                    return if state.failed || self.cancelled() {
                        Err(())
                    } else {
                        Ok((state.best, state.best_id))
                    };
                }
            }
            // Help descendants only: unrelated work must not delay this proof
            // or recursively tie up the master of an ancestor split.
            if let Some((s, m, a)) = pool.take(Some(&split.cancel)) {
                self.work(s, m, a);
            } else {
                let queue = pool.queue.lock().unwrap();
                {
                    pool.idle.0.fetch_add(1, Relaxed);
                    let _guard = pool
                        .wake
                        .wait_timeout(queue, Duration::from_micros(100))
                        .unwrap();
                    pool.idle.0.fetch_sub(1, Relaxed);
                }
            }
        }
    }
}

pub(super) fn solve(
    p: Position,
    entries: usize,
    threads: usize,
    deadline: f64,
    prove: bool,
) -> (Option<(i32, Option<Move>)>, u64) {
    execute(entries, threads, deadline, |s| {
        (if prove {
            s.prove_best(p)
        } else {
            s.exact(p, false).map(|v| (v, None))
        })
        .ok()
    })
}

pub(super) fn execute<R>(
    entries: usize,
    threads: usize,
    deadline: f64,
    operation: impl FnOnce(&mut Search) -> R,
) -> (R, u64) {
    let _qos = QosGuard::new();
    if threads <= 1 {
        let mut s = Search::new(entries, deadline);
        s.root_null = true;
        let result = operation(&mut s);
        return (result, s.nodes);
    }
    let table = Arc::new(SharedTable::new(entries));
    let pool = Arc::new(Pool {
        width: std::env::var("DOSELLO_SPLIT_WIDTH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2)
            .max(1),
        min_depth: std::env::var("DOSELLO_SPLIT_DEPTH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(14),
        queue: Mutex::new(VecDeque::new()),
        wake: Condvar::new(),
        idle: Counter(0.into()),
        stop: false.into(),
        splits: Counter(0.into()),
        stolen: Counter(0.into()),
    });
    let mut master = Search::new(4, deadline);
    master.shared = Some(table.clone());
    master.parallel = Some(pool.clone());
    master.root_null = true;
    let (result, workers) = std::thread::scope(|scope| {
        let mut jobs = Vec::new();
        for _ in 1..threads.min(64) {
            let pool = pool.clone();
            let table = table.clone();
            jobs.push(scope.spawn(move || {
                let _qos = QosGuard::new();
                let mut s = Search::new(4, deadline);
                s.shared = Some(table);
                s.parallel = Some(pool.clone());
                loop {
                    if let Some((split, m, a)) = pool.take(None) {
                        s.work(split, m, a);
                        continue;
                    }
                    let queue = pool.queue.lock().unwrap();
                    if pool.stop.load(Relaxed) {
                        break;
                    }
                    pool.idle.0.fetch_add(1, Relaxed);
                    let _guard = pool
                        .wake
                        .wait_timeout(queue, Duration::from_millis(1))
                        .unwrap();
                    pool.idle.0.fetch_sub(1, Relaxed);
                }
                s.nodes
            }));
        }
        let result = operation(&mut master);
        pool.stop.store(true, Relaxed);
        pool.wake.notify_all();
        (
            result,
            jobs.into_iter()
                .map(|j| j.join().unwrap())
                .collect::<Vec<_>>(),
        )
    });
    let nodes = master.nodes + workers.iter().sum::<u64>();
    if std::env::var_os("DOSELLO_PARALLEL_STATS").is_some() {
        eprintln!(
            "parallel splits={} claims={} masterNodes={} workerNodes={:?}",
            pool.splits.0.load(Relaxed),
            pool.stolen.0.load(Relaxed),
            master.nodes,
            workers
        );
    }
    (result, nodes)
}

// Explicit user-requested compute may otherwise inherit a background QoS from
// a terminal/agent host. Restore the caller's QoS when its solve is finished.
struct QosGuard {
    #[cfg(target_os = "macos")]
    previous: Option<(u32, i32)>,
}
#[cfg(target_os = "macos")]
extern "C" {
    fn pthread_self() -> usize;
    fn pthread_get_qos_class_np(thread: usize, class: *mut u32, priority: *mut i32) -> i32;
    fn pthread_set_qos_class_self_np(class: u32, priority: i32) -> i32;
}
impl QosGuard {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        {
            let mut class = 0;
            let mut priority = 0;
            let previous = if std::env::var("DOSELLO_QOS").as_deref() != Ok("0") {
                // SAFETY: valid local out-pointers; only the calling thread is changed.
                unsafe {
                    if pthread_get_qos_class_np(pthread_self(), &mut class, &mut priority) == 0
                        && class != 0
                        && pthread_set_qos_class_self_np(0x19, 0) == 0
                    {
                        Some((class, priority))
                    } else {
                        None
                    }
                }
            } else {
                None
            };
            Self { previous }
        }
        #[cfg(not(target_os = "macos"))]
        Self {}
    }
}
impl Drop for QosGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if let Some((class, priority)) = self.previous {
            // SAFETY: restore the previously returned class for this thread.
            unsafe {
                pthread_set_qos_class_self_np(class, priority);
            }
        }
    }
}
