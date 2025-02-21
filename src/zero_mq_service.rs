use std::collections::VecDeque;

use std::sync::Arc;
use std::task::Poll;

use crate::db::{connection::DbHandler, models::ClassificationSerde};
use anyhow::{Ok, Result};
use futures::Future;
use log::{debug, error};
use tokio::sync::mpsc::{self};
use tokio::sync::Mutex;
use tokio::task;
use zeromq::{PubSocket, Socket, SocketRecv, SocketSend, SubSocket};

pub struct Publisher {
    context: Mutex<PubSocket>,
    queue: Mutex<VecDeque<ClassificationSerde>>,
}

#[derive(Clone)]
pub struct RecvFuture {
    pub recv: Arc<Mutex<mpsc::UnboundedReceiver<bool>>>,
}

impl RecvFuture {
    pub fn new(receiver: mpsc::UnboundedReceiver<bool>) -> Self {
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
        let mut ctx = PubSocket::new();
        if let Err(e) = ctx.bind("tcp://127.0.0.1:30002").await {
            error!("Unable to bind Zeromq Tcp socket: {}", e);
        }
        Arc::new(Self {
            context: Mutex::new(ctx),
            queue: Mutex::new(VecDeque::with_capacity(50)),
        })
    }

    async fn send_classification_content(
        &self,
        classification: &ClassificationSerde,
    ) -> Result<()> {
        match serde_json::to_string(&classification) {
            std::result::Result::Ok(classification_json) => {
                if let Err(e) = self
                    .context
                    .lock()
                    .await
                    .send(classification_json.into())
                    .await
                {
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
        }
        Ok(())
    }

    async fn remove_task_from_queue(self: Arc<Self>, val: &ClassificationSerde) -> Result<()> {
        let mut queue = self.queue.lock().await;
        queue.retain(|task| task != val);
        Ok(())
    }
    pub async fn call_classifier_agent(
        self: Arc<Self>,
        db_handler: Arc<DbHandler>,
        recv: RecvFuture,
    ) -> Result<()> {
        loop {
            self.clone().update_task_queue(db_handler.clone()).await?;
            let queue = self.queue.lock().await;
            let mut jhs = vec![];
            let total_task = queue.len();
            debug!("Total tasks to process: {}", queue.len());
            //TODO: add another channel to recv that condition is true or use an alternative approach
            while let Some(true) = recv.next().await {
                for (i, val) in queue.clone().into_iter().enumerate() {
                    let self_clone = Arc::clone(&self);

                    debug!("Spawning task {}/{}", i + 1, total_task);
                    while let Some(false) = recv.next().await {}

                    let jh = task::spawn(async move {
                        debug!("Processing value: {:?}", val);

                        if let Err(e) = self_clone.send_classification_content(&val).await {
                            error!("Failed to process classification: {}", e);
                        }
                        let _ = self_clone.clone().remove_task_from_queue(&val).await;
                    });

                    jhs.push(jh);
                }
            }
            let mut completed_tasks = 0;
            for jh in jhs {
                if jh.await.is_err() {
                    error!("[ERROR] Task failed to complete.");
                }
                completed_tasks += 1;
                debug!("Completed task {}/{}", completed_tasks, total_task);
            }

            debug!("All tasks completed. Waiting for recv to become true again...");
        }
    }
}

pub struct Subscriber {
    pub subscriber: Mutex<SubSocket>,
}

impl Subscriber {
    pub fn new() -> Arc<Self> {
        let ctx = SubSocket::new();
        Arc::new(Self {
            subscriber: Mutex::new(ctx)
        })
    }

    pub async fn recv_message(self: Arc<Self>, db_handler: Arc<DbHandler>) -> Result<()> {
        let mut ctx = self.subscriber.lock().await;
        if let Err(e) = ctx.connect("tcp://127.0.0.1:30003").await {
            error!("Unable to bind Zeromq Tcp socket: {}", e);
        }

        ctx.subscribe("").await?;
        loop {
            let zmq_message = ctx.recv().await?;
            let vec_bytes = zmq_message.into_vec();

            let bytes_as_u8: Vec<u8> = vec_bytes.into_iter().flat_map(|b| b.to_vec()).collect();
            let escaped_json = String::from_utf8(bytes_as_u8)?;
            let unescaped = escaped_json
            .replace("\\\\", "\\")
            .replace("\\\"", "\"");
            let cleaned = unescaped.trim_matches('"');
            let data = serde_json::from_str::<ClassificationSerde>(&cleaned).unwrap();
            db_handler.update_classification(data).await?;
        }
    }
}

