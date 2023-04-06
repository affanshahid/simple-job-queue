use redis::{ErrorKind, FromRedisValue, RedisError, ToRedisArgs};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

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

#[cfg(feature = "redis")]
impl<T> ToRedisArgs for Job<T>
where
    T: Serialize,
{
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        out.write_arg(&serde_json::to_vec(self).expect("Unable to serialize job"))
    }
}

#[cfg(feature = "redis")]
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
