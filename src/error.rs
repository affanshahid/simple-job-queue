use std::{error::Error, fmt::Display};

use redis::RedisError;

#[derive(Debug)]
pub struct JobError;

impl Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt("An error occurred while processing the job", f)
    }
}

impl Error for JobError {}

#[derive(Debug)]
pub enum JobQueueError {
    #[cfg(feature = "redis")]
    RedisError(RedisError),
}

impl Display for JobQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "redis")]
            JobQueueError::RedisError(error) => error.fmt(f),
        }
    }
}

impl Error for JobQueueError {}

#[cfg(feature = "redis")]
impl From<RedisError> for JobQueueError {
    fn from(value: RedisError) -> Self {
        Self::RedisError(value)
    }
}
