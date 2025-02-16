use std::{error::Error, sync::Arc};

use crate::db::{connection::DbHandler, models::ClassificationSerde};
use anyhow::{Ok, Result};
use log::{error, info};
use tokio::sync::Mutex;
use tokio::{
    sync::Semaphore,
    task::{self},
};
use zeromq::{PubSocket, Socket, SocketSend};

pub struct Publisher {
    context: Mutex<PubSocket>,
}

impl Publisher {
    pub async fn new() -> Arc<Self> {
        let mut ctx = PubSocket::new();
        if let Err(e) = ctx.bind("tcp://127.0.0.1:30002").await {
            error!("Unable to bind Zeromq Tcp socket: {}", e);
        }
        Arc::new(Self { context: Mutex::new(ctx) })
    }

    async fn send_classification_content(&self, classification: ClassificationSerde) -> Result<()> {
        match serde_json::to_string(&classification) {
            std::result::Result::Ok(classification_json) => {
                if let Err(e) = self.context.lock().await.send(classification_json.into()).await {
                    error!("Failed to send classification content: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to serialize classification: {}", e);
            }
        }
        Ok(())
    }

    pub async fn call_classifier_agent(
        self: Arc<Self>,
        db_handler: Arc<DbHandler>,
    ) -> Result<()> {
        let result = DbHandler::fetch_all_classification(&db_handler).await?;
        let semaphore = Arc::new(Semaphore::new(5));
        let total_tasks = result.len();
        let mut jhs = vec![];

        info!("[DEBUG] Total tasks to process: {}", total_tasks);

        for (i, val) in result.into_iter().enumerate() {
            let semaphore = semaphore.clone();
            let self_clone = Arc::clone(&self);

            info!("[DEBUG] Spawning task {}/{}", i + 1, total_tasks);

            let jh = task::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                info!("[DEBUG] Processing value: {:?}", val);
                
                if let Err(e) = self_clone.send_classification_content(val).await {
                    error!("[ERROR] Failed to process classification: {}", e);
                }

                drop(_permit);
            });

            jhs.push(jh);
        }

        let mut completed_tasks = 0;
        for jh in jhs {
            if jh.await.is_err() {
                error!("[ERROR] Task failed to complete.");
            }
            completed_tasks += 1;
            info!("[DEBUG] Completed task {}/{}", completed_tasks, total_tasks);
        }

        info!("[DEBUG] All tasks completed.");
        Ok(())
    }
}
