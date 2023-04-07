use std::{marker::PhantomData, time::Duration};

use async_trait::async_trait;
use tokio::task::JoinHandle;

use crate::{
    error::{JobError, JobQueueError},
    Job,
};

#[async_trait]
pub trait JobQueueBackend<T>: Clone {
    type Context: Send;

    async fn setup(&self) -> Result<(), JobQueueError>;
    async fn produce(&self, job: Job<T>) -> Result<(), JobQueueError>;
    async fn consume(&self) -> Result<(Job<T>, Self::Context), JobQueueError>;
    async fn done(&self, job: Job<T>, ctx: Self::Context);
    async fn failed(&self, job: Job<T>, ctx: Self::Context);
}

#[async_trait]
pub trait Processor<T> {
    async fn process(&mut self, job: &Job<T>) -> Result<(), JobError>;
}

struct JobQueueWorker<T, B, P>
where
    B: JobQueueBackend<T>,
    P: Processor<T>,
{
    backend: B,
    processor: P,
    _t: PhantomData<T>,
}

impl<T, B, P> JobQueueWorker<T, B, P>
where
    B: JobQueueBackend<T>,
    P: Processor<T>,
{
    async fn start(&mut self) -> () {
        loop {
            match self.backend.consume().await {
                Ok((job, ctx)) => match self.processor.process(&job).await {
                    Ok(_) => self.backend.done(job, ctx).await,
                    Err(_) => self.backend.failed(job, ctx).await,
                },
                Err(_) => {
                    // TODO: Make configurable
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            };
        }
    }
}

pub struct JobQueue<T, B>
where
    B: JobQueueBackend<T>,
{
    backend: B,
    worker_handle: Option<JoinHandle<()>>,
    _t: PhantomData<T>,
}

impl<T, B> JobQueue<T, B>
where
    B: JobQueueBackend<T>,
{
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            worker_handle: None,
            _t: PhantomData,
        }
    }

    pub async fn submit(&self, job: Job<T>) -> Result<(), JobQueueError> {
        self.backend.produce(job).await?;

        Ok(())
    }
}

impl<T, B> JobQueue<T, B>
where
    T: Send + Sync + 'static,
    B: JobQueueBackend<T> + Send + Sync + 'static,
{
    pub async fn start<P>(&mut self, processor: P) -> Result<(), JobQueueError>
    where
        P: Processor<T> + Send + Sync + 'static,
    {
        self.backend.setup().await?;

        let mut worker = JobQueueWorker {
            backend: self.backend.clone(),
            processor,
            _t: PhantomData,
        };

        let handle = tokio::spawn(async move { worker.start().await });
        self.worker_handle = Some(handle);

        Ok(())
    }
}

impl<T, B> Drop for JobQueue<T, B>
where
    B: JobQueueBackend<T>,
{
    fn drop(&mut self) {
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }
    }
}
