//! Private bounded recovery proof workers. Only immutable AFTER Game snapshots
//! cross threads; SQLite, input decoding and speculative replay stay on main.
use crate::store::Head;
use mtg_core::Game;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
};

pub(crate) const BATCH_SIZE: usize = 6;
const LANES: usize = 3;

pub(crate) struct AfterHashJob {
    pub(crate) expected: Head,
    pub(crate) game: Game,
}
struct ActualHash {
    sequence: u64,
    boundary: u64,
    digest: Result<String, String>,
}
enum Processed {
    Complete(ActualHash),
    #[cfg(test)]
    Disconnected,
}
type Processor = Arc<dyn Fn(AfterHashJob) -> Processed + Send + Sync>;
type Task = Box<dyn FnOnce() + Send + 'static>;
struct Lane {
    input: Option<SyncSender<AfterHashJob>>,
    output: Receiver<ActualHash>,
    thread: Option<JoinHandle<()>>,
}
pub(crate) struct AfterHashPool {
    lanes: Vec<Lane>,
    pending: Vec<Head>,
    stopped: bool,
}
fn actual_hash(job: AfterHashJob) -> Processed {
    Processed::Complete(ActualHash {
        sequence: job.expected.sequence,
        boundary: job.game.rule_boundary(),
        digest: job.game.state_digest(),
    })
}
fn run_lane(input: Receiver<AfterHashJob>, output: SyncSender<ActualHash>, process: Processor) {
    while let Ok(job) = input.recv() {
        let sequence = job.expected.sequence;
        let boundary = job.expected.boundary;
        // Hashing reads an owned frozen snapshot. OnceLock installs a complete
        // canonical byte result atomically; an unwinding initializer installs
        // nothing. A panic is a failed proof, never a missing result or Ready.
        let result = catch_unwind(AssertUnwindSafe(|| process(job))).unwrap_or_else(|_| {
            Processed::Complete(ActualHash {
                sequence,
                boundary,
                digest: Err("recovery after-State hash worker panicked".into()),
            })
        });
        match result {
            Processed::Complete(result) => {
                if output.send(result).is_err() {
                    break;
                }
            }
            #[cfg(test)]
            Processed::Disconnected => break,
        }
    }
}
impl AfterHashPool {
    pub(crate) fn new() -> Result<Self, String> {
        Self::start(Arc::new(actual_hash), |lane, task| {
            thread::Builder::new()
                .name(format!("mtg-recovery-sha-{lane}"))
                .spawn(task)
                .map_err(|_| "recovery hash worker startup failed".into())
        })
    }
    fn start(
        process: Processor,
        mut spawn: impl FnMut(usize, Task) -> Result<JoinHandle<()>, String>,
    ) -> Result<Self, String> {
        let mut pool = Self {
            lanes: vec![],
            pending: vec![],
            stopped: false,
        };
        for lane in 0..LANES {
            // At most two jobs per lane in a six-job batch. Results capacity
            // covers BOTH, so shutdown can join without draining or blocking
            // a worker on its result send after a main-thread staging error.
            let (input, receiver) = mpsc::sync_channel(1);
            let (sender, output) = mpsc::sync_channel(2);
            let process = process.clone();
            let task = Box::new(move || run_lane(receiver, sender, process));
            match spawn(lane, task) {
                Ok(thread) => pool.lanes.push(Lane {
                    input: Some(input),
                    output,
                    thread: Some(thread),
                }),
                Err(error) => {
                    drop(input);
                    let _ = pool.shutdown();
                    return Err(error);
                }
            }
        }
        Ok(pool)
    }
    pub(crate) fn enqueue(&mut self, job: AfterHashJob) -> Result<(), String> {
        if self.stopped {
            return Err("recovery hash pool already stopped".into());
        }
        if self.pending.len() >= BATCH_SIZE {
            let _ = self.shutdown();
            return Err("recovery hash batch exceeds six proofs".into());
        }
        let lane = self.pending.len() % LANES;
        let expected = job.expected.clone();
        let Some(input) = self.lanes[lane].input.as_ref() else {
            let _ = self.shutdown();
            return Err("recovery hash lane is unavailable".into());
        };
        if input.send(job).is_err() {
            let _ = self.shutdown();
            return Err("recovery hash lane disconnected".into());
        }
        self.pending.push(expected);
        Ok(())
    }
    pub(crate) fn barrier(&mut self) -> Result<(), String> {
        if self.stopped {
            return Err("recovery hash pool already stopped".into());
        }
        let result = self.verify_pending();
        if let Err(error) = result {
            let _ = self.shutdown();
            return Err(error);
        }
        self.pending.clear();
        Ok(())
    }
    fn verify_pending(&self) -> Result<(), String> {
        // Lane FIFO plus this ordered drain proves each original sequence,
        // including partial EOF batches. Different-lane completion order may
        // vary, but duplicate/foreign/missing/bad-boundary results cannot pass.
        for (index, expected) in self.pending.iter().enumerate() {
            let actual = self.lanes[index % LANES]
                .output
                .recv()
                .map_err(|_| "recovery hash result missing or disconnected")?;
            if actual.sequence != expected.sequence || actual.boundary != expected.boundary {
                return Err("recovery hash result identity mismatch".into());
            }
            if actual.digest? != expected.state_sha256 {
                return Err("operation after-State hash mismatch".into());
            }
        }
        Ok(())
    }
    fn shutdown(&mut self) -> Result<(), String> {
        self.stopped = true;
        // Drop ALL senders before the FIRST join. A lane can be idle waiting
        // for input or can own its second queued job. Drain queued work; no
        // error path detaches a thread or silently forgets a proof snapshot.
        for lane in &mut self.lanes {
            lane.input.take();
        }
        let mut panicked = false;
        for lane in &mut self.lanes {
            if let Some(thread) = lane.thread.take() {
                panicked |= thread.join().is_err();
            }
        }
        self.pending.clear();
        if panicked {
            Err("recovery hash worker join failed".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn finish(mut self) -> Result<(), String> {
        self.barrier()?;
        self.shutdown()
    }
}
impl Drop for AfterHashPool {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mtg_contracts::Catalog;
    use mtg_core::{MatchConfig, PlayerId, Prepared};
    use std::sync::{
        Barrier, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    fn game() -> Game {
        let catalog = Catalog::read(include_str!(
            "../../game-engine/tests/fixtures/expected-match-ir.json"
        ))
        .unwrap();
        Game::new(
            Prepared::admit_pod(catalog, MatchConfig::foundations([3; 32], PlayerId(0))).unwrap(),
        )
    }
    fn job(game: &Game, sequence: u64) -> AfterHashJob {
        AfterHashJob {
            expected: Head {
                sequence,
                boundary: game.rule_boundary(),
                state_sha256: game.state_digest().unwrap(),
            },
            game: game.fork(),
        }
    }
    fn pool(process: Processor) -> AfterHashPool {
        AfterHashPool::start(process, |_, task| {
            thread::Builder::new()
                .spawn(task)
                .map_err(|e| e.to_string())
        })
        .unwrap()
    }
    fn counted(count: Arc<AtomicUsize>) -> Processor {
        Arc::new(move |job| {
            count.fetch_add(1, Ordering::SeqCst);
            actual_hash(job)
        })
    }
    #[test]
    fn every_record_in_full_and_partial_batches_is_actually_hashed_even_if_state_identical() {
        let game = game();
        for length in [0, 1, 5, 6, 7, 12, 13] {
            let count = Arc::new(AtomicUsize::new(0));
            let mut pool = pool(counted(count.clone()));
            for index in 1..=length {
                pool.enqueue(job(&game, index)).unwrap();
                assert!(pool.pending.len() <= BATCH_SIZE);
                if index % 6 == 0 {
                    pool.barrier().unwrap();
                }
            }
            pool.finish().unwrap();
            assert_eq!(
                count.load(Ordering::SeqCst),
                usize::try_from(length).unwrap()
            );
        }
    }
    #[test]
    fn three_hash_threads_overlap_and_ordered_barrier_accepts_out_of_order_completion() {
        let game = game();
        let barrier = Arc::new(Barrier::new(3));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(Mutex::new(Vec::new()));
        let (sender, receiver) = mpsc::sync_channel(1);
        let receiver = Mutex::new(receiver);
        let process: Processor = {
            let (barrier, active, peak, completed) = (
                barrier.clone(),
                active.clone(),
                peak.clone(),
                completed.clone(),
            );
            Arc::new(move |job| {
                let sequence = job.expected.sequence;
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                if sequence <= 3 {
                    barrier.wait();
                }
                if sequence == 1 {
                    receiver.lock().unwrap().recv().unwrap();
                }
                let result = actual_hash(job);
                completed.lock().unwrap().push(sequence);
                if sequence == 2 {
                    sender.send(()).unwrap();
                }
                active.fetch_sub(1, Ordering::SeqCst);
                result
            })
        };
        let mut pool = pool(process);
        assert_eq!(pool.lanes.len(), 3);
        for sequence in 1..=6 {
            pool.enqueue(job(&game, sequence)).unwrap();
        }
        assert_eq!(pool.pending.len(), 6);
        pool.finish().unwrap();
        assert_eq!(peak.load(Ordering::SeqCst), 3);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        let completed = completed.lock().unwrap();
        assert_eq!(completed.len(), 6);
        assert!(
            completed.iter().position(|n| *n == 2).unwrap()
                < completed.iter().position(|n| *n == 1).unwrap()
        );
    }
    #[test]
    fn failed_hash_identity_digest_panic_and_disconnect_all_fail_closed_and_join() {
        let game = game();
        for variant in 0..6 {
            let count = Arc::new(AtomicUsize::new(0));
            let process: Processor = {
                let count = count.clone();
                Arc::new(move |job| {
                    count.fetch_add(1, Ordering::SeqCst);
                    if job.expected.sequence != 2 {
                        return actual_hash(job);
                    }
                    if variant == 4 {
                        panic!("injected hash panic");
                    }
                    if variant == 5 {
                        return Processed::Disconnected;
                    }
                    let Processed::Complete(mut result) = actual_hash(job) else {
                        unreachable!()
                    };
                    match variant {
                        0 => result.sequence += 1,
                        1 => result.boundary += 1,
                        2 => result.digest = Ok("0".repeat(64)),
                        3 => result.digest = Err("injected serialization failure".into()),
                        _ => unreachable!(),
                    }
                    Processed::Complete(result)
                })
            };
            let mut pool = pool(process);
            for sequence in 1..=3 {
                pool.enqueue(job(&game, sequence)).unwrap();
            }
            assert!(pool.barrier().is_err());
            assert!(pool.stopped);
            assert!(
                pool.lanes
                    .iter()
                    .all(|lane| lane.input.is_none() && lane.thread.is_none())
            );
            assert_eq!(count.load(Ordering::SeqCst), 3);
            assert!(pool.enqueue(job(&game, 4)).is_err());
            assert!(pool.finish().is_err());
        }
    }
    #[test]
    fn main_staging_error_drop_and_over_capacity_drain_all_six_snapshots() {
        let game = game();
        for overflow in [false, true] {
            let count = Arc::new(AtomicUsize::new(0));
            let mut pool = pool(counted(count.clone()));
            for sequence in 1..=6 {
                pool.enqueue(job(&game, sequence)).unwrap();
            }
            if overflow {
                assert!(pool.enqueue(job(&game, 7)).is_err());
                assert!(pool.lanes.iter().all(|lane| lane.thread.is_none()));
            }
            // This is also the unwind/early-return cleanup used when sequential
            // record staging fails while all six proof jobs are in flight.
            drop(pool);
            assert_eq!(count.load(Ordering::SeqCst), 6);
        }
    }
    #[test]
    fn partial_thread_start_failure_closes_and_joins_every_started_lane() {
        struct Exit(Arc<AtomicUsize>);
        impl Drop for Exit {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        for fail_at in 0..3 {
            let exited = Arc::new(AtomicUsize::new(0));
            let result = AfterHashPool::start(Arc::new(actual_hash), |lane, task| {
                if lane == fail_at {
                    return Err("injected spawn failure".into());
                }
                let exited = exited.clone();
                thread::Builder::new()
                    .spawn(move || {
                        let _exit = Exit(exited);
                        task();
                    })
                    .map_err(|e| e.to_string())
            });
            assert!(result.is_err());
            assert_eq!(exited.load(Ordering::SeqCst), fail_at);
        }
    }
}
