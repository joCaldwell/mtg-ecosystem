//! Bounded match-local serialization. A queued cancellation removes the exact
//! unsent submission before sending a durable tombstone to the one worker.
use crate::{
    ipc::{WorkerCommand, WorkerReply},
    process::{SpawnConfig, WorkerProcess},
    protocol::ExecutionStatus,
    worker::ClockControl,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{Notify, Semaphore, oneshot};

pub(crate) const READER_LIMIT: usize = 4;
type Reader = Arc<crate::resume_reader::ResumeReader>;

// The production await seam also permits a deterministic held-reply regression.
async fn await_worker_reply(
    request: impl std::future::Future<Output = Result<WorkerReply, crate::process::BridgeError>>,
) -> Result<WorkerReply, crate::process::BridgeError> {
    request.await
}

pub const QUEUE_LIMIT: usize = 16;
const CALL_TIMEOUT: Duration = Duration::from_secs(120);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrokerError {
    Busy,
    Unavailable,
}
type Reply = Result<WorkerReply, BrokerError>;
struct Queued {
    command: WorkerCommand,
    replies: Vec<oneshot::Sender<Reply>>,
}
#[derive(Default)]
struct Queue {
    requests: VecDeque<Queued>,
    stopping: bool,
    dead: bool,
}
impl Queue {
    fn next_request(
        &mut self,
        status: &ExecutionStatus,
        served: usize,
        last_clock: tokio::time::Instant,
        now: tokio::time::Instant,
    ) -> Option<Queued> {
        let progress_due = matches!(status, ExecutionStatus::Running) && served >= 4;
        let clock_due = matches!(status, ExecutionStatus::Waiting)
            && now.duration_since(last_clock) >= Duration::from_secs(1);
        if progress_due || clock_due {
            None
        } else {
            self.requests.pop_front()
        }
    }
}
async fn complete_scheduled<T>(
    request: impl std::future::Future<Output = T>,
    last_clock: &mut tokio::time::Instant,
) -> T {
    let result = request.await;
    // Worker busy time is excluded from its action clock. Also exclude it from
    // the broker's next sampling interval: a Tick taking >=1s must not make
    // another Tick immediately due ahead of every queued request.
    *last_clock = tokio::time::Instant::now();
    result
}
#[derive(Clone)]
pub struct Broker {
    queue: Arc<Mutex<Queue>>,
    notify: Arc<Notify>,
    reader: Arc<Mutex<Option<Reader>>>,
    reader_admission: Arc<Semaphore>,
}
impl Broker {
    pub fn start(process: WorkerProcess) -> (Self, tokio::task::JoinHandle<()>) {
        Self::start_inner(process, None)
    }
    pub fn start_recovering(
        process: WorkerProcess,
        config: SpawnConfig,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        Self::start_inner(process, Some(config))
    }
    fn start_inner(
        process: WorkerProcess,
        config: Option<SpawnConfig>,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let broker = Self {
            queue: Arc::new(Mutex::new(Queue::default())),
            notify: Arc::new(Notify::new()),
            reader: Arc::new(Mutex::new(Some(process.resume_reader()))),
            reader_admission: Arc::new(Semaphore::new(READER_LIMIT)),
        };
        let actor = broker.clone();
        let task = tokio::spawn(async move { actor.run(process, config).await });
        (broker, task)
    }
    fn enqueue(&self, command: WorkerCommand) -> Result<oneshot::Receiver<Reply>, BrokerError> {
        let (sender, receiver) = oneshot::channel();
        {
            let mut queue = self.queue.lock().map_err(|_| BrokerError::Unavailable)?;
            if queue.dead || queue.stopping {
                return Err(BrokerError::Unavailable);
            }
            if queue.requests.len() >= QUEUE_LIMIT {
                return Err(BrokerError::Busy);
            }
            queue.requests.push_back(Queued {
                command,
                replies: vec![sender],
            });
        }
        self.notify.notify_one();
        Ok(receiver)
    }
    pub async fn call(&self, command: WorkerCommand) -> Reply {
        // Both HTTP and the lifecycle projector use this call boundary. Pure
        // committed Resume never enters the one-rules-executor queue.
        if let WorkerCommand::Resume { seat } = command {
            return self.resume(seat).await;
        }
        self.enqueue(command)?
            .await
            .unwrap_or(Err(BrokerError::Unavailable))
    }
    async fn resume(&self, seat: u8) -> Reply {
        let permit = self
            .reader_admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| BrokerError::Busy)?;
        let reader = self
            .reader
            .lock()
            .map_err(|_| BrokerError::Unavailable)?
            .clone()
            .ok_or(BrokerError::Unavailable)?;
        let epoch = reader.clone();
        let result = tokio::task::spawn_blocking(move || {
            // Admission follows the blocking job even if its caller is dropped.
            let _permit = permit;
            reader.read(seat)
        })
        .await
        .map_err(|_| BrokerError::Unavailable)?;
        let gate = self.reader.lock().map_err(|_| BrokerError::Unavailable)?;
        if !gate
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &epoch))
        {
            return Err(BrokerError::Unavailable);
        }
        result
            .map(|resume| WorkerReply::Resume {
                resume: Box::new(resume),
            })
            .map_err(|_| BrokerError::Unavailable)
    }
    fn revoke_reader(&self) {
        if let Ok(mut gate) = self.reader.lock() {
            if let Some(reader) = gate.take() {
                reader.revoke();
            }
        }
    }
    fn install_reader(&self, process: &WorkerProcess) -> Option<()> {
        let queue = self.queue.lock().ok()?;
        if queue.stopping || queue.dead {
            return None;
        }
        *self.reader.lock().ok()? = Some(process.resume_reader());
        Some(())
    }
    /// None means the submission has already left this queue (or never existed).
    /// The caller may inspect its durable execution status, but cannot undo it.
    pub async fn cancel(&self, seat: u8, id: &str) -> Result<Option<WorkerReply>, BrokerError> {
        let receiver = {
            let mut queue = self.queue.lock().map_err(|_| BrokerError::Unavailable)?;
            if queue.dead || queue.stopping {
                return Err(BrokerError::Unavailable);
            }
            let index = queue.requests.iter().position(|item| matches!(&item.command, WorkerCommand::Submit {seat: owner, request} if *owner==seat && request.command_id==id));
            let Some(index) = index else {
                return Ok(None);
            };
            let mut item = queue
                .requests
                .remove(index)
                .expect("position exists under same lock");
            let WorkerCommand::Submit { seat, request } = item.command else {
                unreachable!()
            };
            item.command = WorkerCommand::CancelQueued { seat, request };
            let (sender, receiver) = oneshot::channel();
            item.replies.push(sender);
            // The tombstone precedes every later queued command. The original
            // caller and canceller receive one identical durable result.
            queue.requests.push_front(item);
            receiver
        };
        self.notify.notify_one();
        receiver
            .await
            .unwrap_or(Err(BrokerError::Unavailable))
            .map(Some)
    }
    pub fn stop(&self) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.stopping = true;
        }
        self.revoke_reader();
        self.notify.notify_one();
    }
    fn finish(&self) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.dead = true;
            for item in queue.requests.drain(..) {
                for sender in item.replies {
                    let _ = sender.send(Err(BrokerError::Unavailable));
                }
            }
        }
        self.revoke_reader();
    }
    async fn recover(
        &self,
        old: WorkerProcess,
        config: &Option<SpawnConfig>,
        attempts: &mut usize,
    ) -> Option<WorkerProcess> {
        self.revoke_reader();
        // Never replace a writer whose resource group was not verified empty.
        if old.shutdown(Duration::from_secs(5)).await.is_err() {
            return None;
        }
        let mut config = config.clone()?;
        config.config = None;
        while *attempts < 3 {
            if self.queue.lock().ok()?.stopping {
                return None;
            }
            *attempts += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
            if let Ok(process) =
                WorkerProcess::spawn(config.clone(), Duration::from_secs(120)).await
            {
                return Some(process);
            }
        }
        None
    }
    async fn run(self, mut process: WorkerProcess, config: Option<SpawnConfig>) {
        let mut status = process.status().clone();
        let mut served = 0_usize;
        let mut recoveries = 0_usize;
        let mut last_clock = tokio::time::Instant::now();
        loop {
            // Register before inspecting the queue so enqueue cannot lose wakeup.
            let notified = self.notify.notified();
            let item = {
                let Ok(mut queue) = self.queue.lock() else {
                    break;
                };
                if queue.stopping {
                    break;
                }
                queue.next_request(&status, served, last_clock, tokio::time::Instant::now())
            };
            let command = if let Some(item) = item {
                if matches!(item.command, WorkerCommand::QueryActions { .. })
                    && item.replies.iter().all(oneshot::Sender::is_closed)
                {
                    continue;
                }
                let result = await_worker_reply(process.call(item.command, CALL_TIMEOUT))
                    .await
                    .map_err(|_| BrokerError::Unavailable);
                if let Ok(reply) = &result
                    && let Some(next) = reply.status()
                {
                    status = next.clone();
                }
                served = served.saturating_add(1);
                let failed = result.is_err()
                    || matches!(
                        &result,
                        Ok(WorkerReply::Error {
                            code: crate::ipc::WorkerErrorCode::Unavailable
                        })
                    );
                if failed {
                    self.revoke_reader();
                }
                for sender in item.replies {
                    let _ = sender.send(result.clone());
                }
                if failed {
                    match self.recover(process, &config, &mut recoveries).await {
                        Some(recovered) => {
                            process = recovered;
                            if self.install_reader(&process).is_none() {
                                break;
                            }
                            status = process.status().clone();
                            served = 0;
                            last_clock = tokio::time::Instant::now();
                        }
                        None => {
                            self.finish();
                            return;
                        }
                    }
                }
                continue;
            } else if matches!(status, ExecutionStatus::Running) {
                WorkerCommand::Advance
            } else {
                if !matches!(status, ExecutionStatus::Waiting) {
                    notified.await;
                    continue;
                }
                if last_clock.elapsed() < Duration::from_secs(1) {
                    tokio::select! {
                        () = notified => continue,
                        () = tokio::time::sleep_until(last_clock + Duration::from_secs(1)) => {}
                    }
                }
                if !matches!(status, ExecutionStatus::Waiting) {
                    continue;
                }
                WorkerCommand::Clock {
                    control: ClockControl::Tick,
                }
            };
            served = 0;
            match complete_scheduled(
                await_worker_reply(process.call(command, CALL_TIMEOUT)),
                &mut last_clock,
            )
            .await
            {
                Ok(reply) if !matches!(reply, WorkerReply::Error { .. }) => {
                    if let Some(next) = reply.status() {
                        status = next.clone();
                    }
                }
                _ => match self.recover(process, &config, &mut recoveries).await {
                    Some(recovered) => {
                        process = recovered;
                        if self.install_reader(&process).is_none() {
                            break;
                        }
                        status = process.status().clone();
                        last_clock = tokio::time::Instant::now();
                    }
                    None => {
                        self.finish();
                        return;
                    }
                },
            }
        }
        self.finish();
        let _ = process.shutdown(Duration::from_secs(5)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{CommandBody, CommandEnvelope, Counter};
    fn submission(id: u64) -> WorkerCommand {
        WorkerCommand::Submit {
            seat: 0,
            request: CommandEnvelope {
                protocol_version: 1,
                command_id: format!("{id:032x}"),
                expected_revision: Counter::new(0),
                command: CommandBody::Concede,
            },
        }
    }
    fn idle() -> Broker {
        Broker {
            queue: Arc::new(Mutex::new(Queue::default())),
            notify: Arc::new(Notify::new()),
            reader: Arc::new(Mutex::new(None)),
            reader_admission: Arc::new(Semaphore::new(READER_LIMIT)),
        }
    }
    #[tokio::test]
    async fn slow_clock_completion_services_queued_query_before_another_tick() {
        let broker = idle();
        let _receiver = broker
            .enqueue(WorkerCommand::QueryActions {
                seat: 0,
                request: crate::protocol::ActionQueryRequest::Continue {
                    cursor: "a".repeat(64),
                    work: 1,
                },
            })
            .unwrap();
        let mut last_clock = tokio::time::Instant::now() - Duration::from_secs(2);
        assert!(
            broker
                .queue
                .lock()
                .unwrap()
                .next_request(
                    &ExecutionStatus::Waiting,
                    0,
                    last_clock,
                    tokio::time::Instant::now()
                )
                .is_none()
        );
        // Exercise the same completion path as the real automatic worker call.
        // Starting the interval before this slow call leaves the queue starved.
        complete_scheduled(
            async {
                tokio::time::sleep(Duration::from_millis(1_050)).await;
            },
            &mut last_clock,
        )
        .await;
        let finished = tokio::time::Instant::now();
        let item = broker
            .queue
            .lock()
            .unwrap()
            .next_request(&ExecutionStatus::Waiting, 0, last_clock, finished)
            .expect("slow clock must leave a service interval for the queued query");
        assert!(matches!(item.command, WorkerCommand::QueryActions { .. }));
        assert_eq!(item.replies.len(), 1);
        assert!(!item.replies[0].is_closed());
        broker.enqueue(submission(8)).unwrap();
        assert!(
            broker
                .queue
                .lock()
                .unwrap()
                .next_request(
                    &ExecutionStatus::Waiting,
                    0,
                    last_clock,
                    finished + Duration::from_secs(1)
                )
                .is_none(),
            "clock sampling still becomes due after the service interval"
        );
        assert_eq!(broker.queue.lock().unwrap().requests.len(), 1);
    }
    #[test]
    fn running_quantum_keeps_its_existing_priority_over_queued_work() {
        let broker = idle();
        let _receiver = broker.enqueue(submission(9)).unwrap();
        let now = tokio::time::Instant::now();
        assert!(
            broker
                .queue
                .lock()
                .unwrap()
                .next_request(&ExecutionStatus::Running, 4, now, now)
                .is_none()
        );
        assert!(
            broker
                .queue
                .lock()
                .unwrap()
                .next_request(&ExecutionStatus::Running, 3, now, now)
                .is_some()
        );
    }
    #[tokio::test]
    async fn queue_bounds_are_reversible_and_shutdown_rejects_unacknowledged_work() {
        let broker = idle();
        let mut receivers = Vec::new();
        for id in 0..QUEUE_LIMIT {
            receivers.push(broker.enqueue(submission(id as u64)).unwrap());
        }
        assert!(matches!(
            broker.enqueue(submission(99)),
            Err(BrokerError::Busy)
        ));
        broker.finish();
        for receiver in receivers {
            assert_eq!(
                receiver.await.unwrap().unwrap_err(),
                BrokerError::Unavailable
            );
        }
        assert!(matches!(
            broker.enqueue(submission(100)),
            Err(BrokerError::Unavailable)
        ));
    }
    #[tokio::test]
    async fn cancellation_replaces_only_unsent_owner_command_and_shares_committed_result() {
        let broker = idle();
        let original = broker.enqueue(submission(7)).unwrap();
        assert!(
            broker
                .cancel(1, &format!("{:032x}", 7))
                .await
                .unwrap()
                .is_none()
        );
        let cancel = {
            let broker = broker.clone();
            tokio::spawn(async move { broker.cancel(0, &format!("{:032x}", 7)).await })
        };
        tokio::task::yield_now().await;
        let item = broker.queue.lock().unwrap().requests.pop_front().unwrap();
        assert!(matches!(
            item.command,
            WorkerCommand::CancelQueued { seat: 0, .. }
        ));
        assert!(
            broker
                .cancel(0, &format!("{:032x}", 7))
                .await
                .unwrap()
                .is_none()
        );
        let result = WorkerReply::Submission {
            result: crate::protocol::SubmissionResult::Rejected {
                command_id: format!("{:032x}", 7),
                reason: crate::protocol::RejectionReason::Cancelled,
            },
            status: ExecutionStatus::Waiting,
        };
        for sender in item.replies {
            sender.send(Ok(result.clone())).unwrap();
        }
        assert_eq!(original.await.unwrap().unwrap(), result);
        assert_eq!(cancel.await.unwrap().unwrap(), Some(result));
    }
}
