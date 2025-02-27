use std::collections::VecDeque;

use std::sync::Arc;
use std::task::Poll;

use crate::db::{connection::DbHandler, models::ClassificationSerde};
use anyhow::{Ok, Result};
use futures::Future;
use log::{debug, error};
use tokio::sync::mpsc::{self};
use tokio::sync::Mutex;
use tokio::time::sleep;
pub struct Publisher {
    pub context: Mutex<zmq::Socket>,
    pub queue: Mutex<VecDeque<ClassificationSerde>>,
}

#[derive(Clone)]
pub struct RecvFuture {
    pub recv: Arc<Mutex<mpsc::Receiver<bool>>>,
}

impl RecvFuture {
    pub fn new(receiver: mpsc::Receiver<bool>) -> Self {
        RecvFuture {
            recv: Arc::new(Mutex::new(receiver)),
        }
    }

    pub async fn next(&self) -> Option<bool> {
        let mut recv = self.recv.lock().await;
        recv.recv().await
    }
}
impl Future for RecvFuture {
    type Output = bool;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut recv = match self.recv.try_lock() {
            std::result::Result::Ok(guard) => guard,
            Err(_) => return Poll::Pending,
        };
        match recv.poll_recv(cx) {
            std::task::Poll::Pending => std::task::Poll::Ready(false),
            std::task::Poll::Ready(Some(false)) => std::task::Poll::Pending,
            std::task::Poll::Ready(Some(true)) => std::task::Poll::Ready(true),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(false),
        }
    }
}

impl Publisher {
    pub async fn new() -> Arc<Self> {
        let ctx = zmq::Context::new();
        let publisher = ctx.socket(zmq::PUB).unwrap();
        if let Err(e) = publisher.bind("tcp://127.0.0.1:30002") {
            error!("Unable to bind Zeromq Tcp socket: {}", e);
        }
        Arc::new(Self {
            context: Mutex::new(publisher),
            queue: Mutex::new(VecDeque::with_capacity(50)),
        })
    }

    async fn send_classification_content(
        &self,
        classification: &ClassificationSerde,
    ) -> Result<()> {
        match serde_json::to_string(&classification) {
            std::result::Result::Ok(classification_json) => {
                if let Err(e) = self.context.lock().await.send(&classification_json, 0) {
                    error!("Failed to send classification content: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to serialize classification: {}", e);
            }
        }
        Ok(())
    }

    async fn update_task_queue(self: Arc<Self>, db_handler: Arc<DbHandler>) -> Result<()> {
        let mut queue = self.queue.lock().await;
        if queue.is_empty() {
            let mut new_tasks = DbHandler::fetch_all_classification(&db_handler).await?;
            queue.append(&mut new_tasks);
            drop(new_tasks)
        }
        Ok(())
    }

    async fn is_queue_empty(self: Arc<Self>) -> bool {
        self.queue.lock().await.is_empty()
    }

    async fn remove_task_from_queue(self: Arc<Self>) -> Option<ClassificationSerde> {
        let mut queue = self.queue.lock().await;
        queue.pop_front()
    }

    pub async fn call_classifier_agent(
        self: Arc<Self>,
        db_handler: Arc<DbHandler>,
        recv: RecvFuture,
    ) -> Result<()> {
        self.clone().update_task_queue(db_handler.clone()).await?;
        loop {
            if let Some(true) = recv.next().await {
                let value = self.clone().remove_task_from_queue().await.unwrap();
                let self_clone = Arc::clone(&self);
                if let Err(e) = self_clone.send_classification_content(&value).await {
                    error!("Failed to process classification: {}", e);
                }
                if self.clone().is_queue_empty().await {
                    self.clone().update_task_queue(db_handler.clone()).await?;
                    debug!("All tasks completed. Waiting for recv to become true again...");
                }
            } else {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

pub struct Subscriber {
    pub subscriber: Mutex<zmq::Socket>,
}

impl Subscriber {
    pub fn new() -> Arc<Self> {
        let ctx = zmq::Context::new();
        let sub = ctx.socket(zmq::SUB).unwrap();
        Arc::new(Self {
            subscriber: Mutex::new(sub),
        })
    }

    pub async fn recv_message(self: Arc<Self>, db_handler: Arc<DbHandler>) -> Result<()> {
        let ctx = self.subscriber.lock().await;
        if let Err(e) = ctx.connect("tcp://127.0.0.1:30003") {
            error!("Unable to bind Zeromq Tcp socket: {}", e);
        }

        if let Err(e) = ctx.set_subscribe(b"") {
            error!("Unable to bind Zeromq Tcp socket: {}", e);
        }
        loop {
            match ctx.recv_string(0) {
                std::result::Result::Ok(zmq_message) => {
                    let message = zmq_message.unwrap();
                    let unescaped = message.replace("\\\\", "\\").replace("\\\"", "\"");
                    let cleaned = unescaped.trim_matches('"');
                    let data = serde_json::from_str::<ClassificationSerde>(&cleaned).unwrap();

                    db_handler.update_classification(data).await?;
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);
                    sleep(tokio::time::Duration::from_millis(100)).await; // Prevents high CPU usage on failure
                }
            }
            sleep(tokio::time::Duration::from_millis(1000)).await;
        }
    }
}
