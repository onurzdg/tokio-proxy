use async_trait::async_trait;
use std::time::Duration;
use std::io;
use std::io::ErrorKind;
use tokio::net::TcpStream;
use tokio::time::timeout;
use crate::asyc_read_write::{Readable, Writable};

#[async_trait]
pub trait TargetConnectionProvider {
    type ReadableWritable: Readable + Writable;
    async fn connect(&self, target: &str, duration: Duration)
                     -> io::Result<Self::ReadableWritable>;
}

pub struct DefaultTargetConnectionProvider;

#[async_trait]
impl TargetConnectionProvider for DefaultTargetConnectionProvider {
    type ReadableWritable = TcpStream;

    async fn connect(
        &self,
        target: &str,
        duration: Duration,
    ) -> io::Result<Self::ReadableWritable> {
        let tcp_steam_result_with_timeout = timeout(duration, TcpStream::connect(target)).await;
        match tcp_steam_result_with_timeout {
            Ok(tcp_steam_result) => tcp_steam_result,
            Err(_) => Err(std::io::Error::from(ErrorKind::TimedOut)),
        }
    }
}