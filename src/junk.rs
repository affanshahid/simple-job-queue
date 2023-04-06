impl<T> FromRedisValue for Job<T>
where
    T: Serialize + DeserializeOwned,
{
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        match *v {
            Value::Data(ref bytes) => {
                serde_json::from_str(from_utf8(bytes)?).or(Err(RedisError::from((
                    ErrorKind::TypeError,
                    "Unable to deserialize",
                    format!("{:?} (response was {:?})", "Unable to deserialize", v),
                ))))
            }
            _ => Err(RedisError::from((
                ErrorKind::TypeError,
                "Response was of incompatible type",
                format!(
                    "{:?} (response was {:?})",
                    "Response was of incompatible type", v
                ),
            ))),
        }
    }
}

impl<T> ToRedisArgs for Job<T>
where
    T: Serialize + DeserializeOwned,
{
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        out.write_arg(
            serde_json::to_string(self)
                .expect("Could not serialize")
                .as_bytes(),
        )
    }
}
