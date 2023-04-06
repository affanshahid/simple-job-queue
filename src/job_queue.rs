use std::marker::PhantomData;

use async_trait::async_trait;
use redis::{
    aio::Connection, AsyncCommands, Client, Direction, ErrorKind, FromRedisValue,
    IntoConnectionInfo, RedisError, ToRedisArgs,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::{JobError, JobQueueError};

const MAIN_QUEUE: &str = "main";
const WORKER_QUEUE: &str = "worker";

#[derive(Serialize, Deserialize)]
pub struct Job<T> {
    pub id: Uuid,
    pub data: T,
}

impl<T> Job<T> {
    pub fn new(data: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            data,
        }
    }
}

impl<T> ToRedisArgs for Job<T>
where
    T: Serialize,
{
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        out.write_arg(&serde_json::to_vec(self).expect("serialization failed"));
    }
}

impl<T> FromRedisValue for Job<T>
where
    T: DeserializeOwned,
{
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        match v {
            redis::Value::Data(data) => serde_json::from_slice(&data).map_err(|e| {
                RedisError::from((
                    ErrorKind::TypeError,
                    "JSON conversion failed.",
                    e.to_string(),
                ))
            }),
            _ => Err(RedisError::from((
                ErrorKind::TypeError,
                "Response type not string compatible.",
            ))),
        }
    }
}

#[async_trait]
pub trait Processor {
    type T;

    async fn process(&mut self, job: Job<Self::T>) -> Result<(), JobError>;
}

struct JobQueueWorker<P>
where
    P: Processor,
{
    connection: Connection,
    name: String,
    processor: P,
}

impl<P> JobQueueWorker<P>
where
    P: Processor,
    P::T: DeserializeOwned,
{
    async fn start(&mut self) -> () {
        loop {
            match self
                .connection
                .blmove(
                    format!("{}_{}", self.name, MAIN_QUEUE),
                    format!("{}_{}", self.name, WORKER_QUEUE),
                    Direction::Right,
                    Direction::Left,
                    0,
                )
                .await
            {
                Ok(job) => match self.processor.process(job).await {
                    Ok(_) => {
                        self.connection
                            .lpop::<_, String>(format!("{}_{}", self.name, WORKER_QUEUE), None)
                            .await
                            .expect("Failed to pop from worker queue");
                    }
                    Err(_) => todo!("Handle retries"),
                },
                Err(_) => todo!("Handle retries"),
            }
        }
    }
}

pub struct JobQueue<T> {
    client: Client,
    name: String,
    worker_handle: Option<JoinHandle<()>>,
    _p: PhantomData<*const T>,
}

impl<T> JobQueue<T> {
    pub fn new<R: IntoConnectionInfo>(
        connection_params: R,
        name: String,
    ) -> Result<Self, JobQueueError> {
        let client = Client::open(connection_params)?;

        Ok(Self {
            client,
            name,
            worker_handle: None,
            _p: PhantomData,
        })
    }
}

impl<T> JobQueue<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    pub async fn start<P>(&mut self, processor: P) -> Result<(), JobQueueError>
    where
        P: Processor<T = T> + Send + Sync + 'static,
    {
        if let Some(_) = self.worker_handle {
            panic!("start called twice");
        }

        let mut worker = JobQueueWorker {
            connection: self.client.get_async_connection().await?,
            name: self.name.clone(),
            processor,
        };

        let handle = tokio::spawn(async move {
            worker.start().await;
        });

        self.worker_handle = Some(handle);

        Ok(())
    }

    pub async fn submit(&self, job: Job<T>) -> Result<(), JobQueueError> {
        let mut conn = self.client.get_async_connection().await?;
        conn.lpush(format!("{}_{}", self.name, MAIN_QUEUE), &job)
            .await?;

        Ok(())
    }
}

impl<T> Drop for JobQueue<T> {
    fn drop(&mut self) {
        if let Some(worker_handle) = self.worker_handle.take() {
            worker_handle.abort();
        }
    }
}
