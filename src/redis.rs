use async_trait::async_trait;
use redis::{AsyncCommands, Client, Direction, IntoConnectionInfo, RedisError};
use serde::{de::DeserializeOwned, Serialize};

use crate::{error::JobQueueError, Job, JobQueueBackend};

const MAIN_QUEUE: &str = "main";
const WORKER_QUEUE: &str = "worker";

#[derive(Clone)]
pub struct RedisJobQueueBackend {
    client: Client,
    name: String,
}

impl RedisJobQueueBackend {
    pub fn new<I: IntoConnectionInfo>(
        connection_info: I,
        name: String,
    ) -> Result<Self, RedisError> {
        Ok(Self {
            client: Client::open(connection_info)?,
            name,
        })
    }

    fn main_queue_key(&self) -> String {
        format!("{}_{}", self.name, MAIN_QUEUE)
    }

    fn worker_queue_key(&self) -> String {
        format!("{}_{}", self.name, WORKER_QUEUE)
    }
}

#[async_trait]
impl<T> JobQueueBackend<T> for RedisJobQueueBackend
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    async fn produce(&self, job: Job<T>) -> Result<(), JobQueueError> {
        let mut conn = self.client.get_async_connection().await?;

        conn.lpush(self.main_queue_key(), &job).await?;

        Ok(())
    }

    async fn consume(&self) -> Result<Job<T>, JobQueueError> {
        let mut conn = self.client.get_async_connection().await?;
        Ok(conn
            .blmove(
                self.main_queue_key(),
                self.worker_queue_key(),
                Direction::Right,
                Direction::Left,
                0,
            )
            .await?)
    }

    async fn done(&self, _: &Job<T>) {
        match self.client.get_async_connection().await {
            Ok(mut conn) => match conn.lpop::<_, String>(self.worker_queue_key(), None).await {
                Ok(_) => (),
                Err(_) => todo!("Handle failure of done notification"),
            },
            Err(_) => todo!("Handle failure of done notification"),
        }
    }

    async fn failed(&self, _: &Job<T>) {
        todo!("Handle job failures")
    }
}
