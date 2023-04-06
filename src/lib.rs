pub mod error;

#[cfg(feature = "redis")]
pub mod job_queue;

#[cfg(feature = "redis")]
pub use job_queue::*;

//////////////////////////////////////////////////
pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
