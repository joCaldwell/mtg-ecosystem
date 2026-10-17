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
use tokio::sync::{Notify, oneshot};

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
#[derive(Clone)]
pub struct Broker {
    queue: Arc<Mutex<Queue>>,
    notify: Arc<Notify>,
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
        self.enqueue(command)?
            .await
            .unwrap_or(Err(BrokerError::Unavailable))
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
    }
    async fn recover(
        &self,
        old: WorkerProcess,
        config: &Option<SpawnConfig>,
        attempts: &mut usize,
    ) -> Option<WorkerProcess> {
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
                let progress_due = matches!(status, ExecutionStatus::Running) && served >= 4;
                let clock_due = matches!(status, ExecutionStatus::Waiting)
                    && last_clock.elapsed() >= Duration::from_secs(1);
                if progress_due || clock_due {
                    None
                } else {
                    queue.requests.pop_front()
                }
            };
            let command = if let Some(item) = item {
                let result = process
                    .call(item.command, CALL_TIMEOUT)
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
                for sender in item.replies {
                    let _ = sender.send(result.clone());
                }
                if failed {
                    match self.recover(process, &config, &mut recoveries).await {
                        Some(recovered) => {
                            process = recovered;
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
            last_clock = tokio::time::Instant::now();
            match process.call(command, CALL_TIMEOUT).await {
                Ok(reply) if !matches!(reply, WorkerReply::Error { .. }) => {
                    if let Some(next) = reply.status() {
                        status = next.clone();
                    }
                }
                _ => match self.recover(process, &config, &mut recoveries).await {
                    Some(recovered) => {
                        process = recovered;
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
        }
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
