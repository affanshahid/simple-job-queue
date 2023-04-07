use std::time::Duration;

use async_trait::async_trait;
use redis::{
    aio::Connection,
    streams::{StreamReadOptions, StreamReadReply},
    AsyncCommands, Client, IntoConnectionInfo, Value,
};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use crate::{error::JobQueueError, Job, JobQueueBackend};

const KEY_DATA: &str = "data";

#[derive(Clone)]
pub struct RedisJobQueueBackendOptions {
    min_idle_time: Duration,
    new_delivery_fetch_timeout: Duration,
    polling_interval: Duration,
}

impl RedisJobQueueBackendOptions {
    pub fn min_idle_time(mut self, d: Duration) -> Self {
        self.min_idle_time = d;
        self
    }

    pub fn new_delivery_fetch_timeout(mut self, d: Duration) -> Self {
        self.new_delivery_fetch_timeout = d;
        self
    }

    pub fn polling_interval(mut self, d: Duration) -> Self {
        self.polling_interval = d;
        self
    }
}

impl Default for RedisJobQueueBackendOptions {
    fn default() -> Self {
        Self {
            min_idle_time: Duration::from_secs(60),
            new_delivery_fetch_timeout: Duration::from_secs(5),
            polling_interval: Duration::from_secs(5),
        }
    }
}

#[derive(Clone)]
pub struct RedisJobQueueBackend {
    client: Client,
    name: String,
    consumer_id: Uuid,
    options: RedisJobQueueBackendOptions,
}

impl RedisJobQueueBackend {
    pub fn new<I: IntoConnectionInfo>(
        connection_info: I,
        name: String,
        options: RedisJobQueueBackendOptions,
    ) -> Result<Self, JobQueueError> {
        Ok(Self {
            client: Client::open(connection_info)?,
            name,
            consumer_id: Uuid::new_v4(),
            options,
        })
    }
}

impl RedisJobQueueBackend {
    async fn read_job<T>(
        &self,
        conn: &mut Connection,
        id: &str,
        block: i64,
    ) -> Result<Option<(Job<T>, RedisJobContext)>, JobQueueError>
    where
        T: DeserializeOwned,
    {
        let mut options = StreamReadOptions::default()
            .group(&self.name, &self.consumer_id.to_string())
            .count(1);

        if block > 0 {
            options = options.block(block as usize);
        }

        conn.xread_options::<_, _, StreamReadReply>(&[&self.name], &[id], &options)
            .await?
            .keys
            .get(0)
            .map(|k| k.ids.get(0))
            .flatten()
            .map(|v| {
                let ctx = RedisJobContext { id: v.id.clone() };
                match v.get(KEY_DATA) {
                    Some(job) => Ok((job, ctx)),
                    None => Err(JobQueueError::MalformedJob),
                }
            })
            .transpose()
    }
}

pub struct RedisJobContext {
    id: String,
}

#[async_trait]
impl<T> JobQueueBackend<T> for RedisJobQueueBackend
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Context = RedisJobContext;

    async fn setup(&self) -> Result<(), JobQueueError> {
        let mut conn = self.client.get_async_connection().await?;
        match conn
            .xgroup_create_mkstream::<_, _, _, String>(&self.name, &self.name, 0)
            .await
        {
            Ok(_) => (),
            Err(err) => match err.code() {
                Some(code) if code == "BUSYGROUP" => (),
                _ => return Err(JobQueueError::from(err)),
            },
        }

        Ok(())
    }

    async fn produce(&self, job: Job<T>) -> Result<(), JobQueueError> {
        let mut conn = self.client.get_async_connection().await?;
        conn.xadd(&self.name, "*", &[(KEY_DATA, job)]).await?;
        Ok(())
    }

    async fn consume(&self) -> Result<(Job<T>, Self::Context), JobQueueError> {
        let mut conn = self.client.get_async_connection().await?;

        redis::cmd("XAUTOCLAIM")
            .arg(&self.name)
            .arg(&self.name)
            .arg(&self.consumer_id.to_string())
            .arg(self.options.min_idle_time.as_millis() as i64)
            .arg(0)
            .arg("COUNT")
            .arg(1)
            .arg("JUSTID")
            .query_async::<_, Value>(&mut conn)
            .await?;

        let mut pending_id = "0".to_string();
        loop {
            let result = self.read_job::<T>(&mut conn, &pending_id, -1).await?;

            match result {
                Some((job, ctx)) if !job.should_process() => {
                    pending_id = ctx.id;
                    continue;
                }
                Some((job, ctx)) => {
                    break Ok((job, ctx));
                }
                None => {
                    match self
                        .read_job::<T>(
                            &mut conn,
                            ">",
                            self.options.new_delivery_fetch_timeout.as_millis() as i64,
                        )
                        .await?
                    {
                        Some((job, _)) if !job.should_process() => {
                            tokio::time::sleep(self.options.polling_interval).await;
                            pending_id = "0".to_string();
                            continue;
                        }
                        Some((job, ctx)) => {
                            break Ok((job, ctx));
                        }
                        None => {
                            pending_id = "0".to_string();
                        }
                    }
                }
            }
        }
    }

    async fn done(&self, _: Job<T>, ctx: Self::Context) {
        match self.client.get_async_connection().await {
            Ok(mut conn) => match conn
                .xack::<_, _, _, Value>(&self.name, &self.name, &[&ctx.id])
                .await
            {
                Ok(_) => match conn.xdel::<_, _, Value>(&self.name, &[ctx.id]).await {
                    Ok(_) => (),
                    Err(_) => todo!("handle done notification failure"),
                },
                Err(_) => todo!("handle done notification failure"),
            },
            Err(_) => todo!("handle done notification failure"),
        }
    }

    async fn failed(&self, _: Job<T>, _: Self::Context) {
        todo!("Handle job failures")
    }
}
