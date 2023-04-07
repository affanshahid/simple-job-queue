# Simple Job Queue

_WIP_

A simple (and probably very ineffecient) async distributed job queue with configurable backends. Built for my own use-case, use at your own peril. Currently only supports [Tokio](https://tokio.rs/).

| Feature             | Redis |
| ------------------- | ----- |
| Job submission      | ✅    |
| Job processing      | ✅    |
| Distributed workers | ✅    |
| Reseliency          | ✅    |
| Delayed execution   | ✅    |
| Retries             | 🟡    |

## Installation

```
cargo add simple-job-queue
```

## Usage

```rust
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simple_job_queue::{
    redis::{RedisJobQueueBackend, RedisJobQueueBackendOptions},
    Job, JobError, JobQueue, JobQueueOptions, Processor,
};

#[derive(Serialize, Deserialize)]
pub struct Data {
    field: i32,
}

pub struct DataProcessor;

#[async_trait]
impl Processor<Data> for DataProcessor {
    async fn process(&mut self, job: &Job<Data>) -> Result<(), JobError> {
        println!("{}", job.data.field);

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let backend = RedisJobQueueBackend::new(
        "redis://:speakfriendandenter@droplet01.affanshahid.dev",
        "queue_name".to_string(),
        RedisJobQueueBackendOptions::default(),
    )
    .unwrap();

    let mut queue: JobQueue<Data, RedisJobQueueBackend> =
        JobQueue::new(backend, JobQueueOptions::default());

    queue.start(DataProcessor).await.unwrap();
    queue.submit(Job::new(Data { field: 1 })).await.unwrap();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();

    queue
        .submit(Job::new_delayed(Data { field: 100 }, now + 10_000))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(15)).await;
}
```
