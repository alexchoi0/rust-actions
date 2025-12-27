use crate::Result;
use std::future::Future;

pub trait World: Sized + Send + Sync + 'static {
    fn new() -> impl Future<Output = Result<Self>> + Send;
}
