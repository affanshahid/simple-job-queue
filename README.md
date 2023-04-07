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
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simple_job_queue::{error::JobError, redis::RedisJobQueueBackend, Job, JobError, JobQueue, Processor};

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
        "redis://127.0.0.1",
        "queue_name".to_string(),
    )
    .unwrap();

    let mut queue: JobQueue<Data, RedisJobQueueBackend> = JobQueue::new(backend);
    queue.start(DataProcessor).await.unwrap();

    queue.submit(Job::new(Data { field: 1 })).await.unwrap();
    queue.submit(Job::new(Data { field: 2 })).await.unwrap();
    queue.submit(Job::new(Data { field: 3 })).await.unwrap();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();

    queue
        .submit(Job::new_delayed(Data { field: 100 }, now + 10_000))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(30)).await;
}
```
