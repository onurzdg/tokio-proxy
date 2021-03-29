use crate::async_read_write::{Pipe, Readable, Writable};
use crate::errors::IoErrorKind;
use serde::Serialize;
use std::io::ErrorKind;
use std::time::Duration;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::time::timeout;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum DataTransferResult {
    Succeeded,
    ConnectionClosed,
    Failed,
    Cancelled,
    Panicked,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct DataTransfer {
    result: DataTransferResult,
    upstream_bytes_received: Option<u64>,
    downstream_bytes_sent: Option<u64>,
    upstream_error: Option<IoErrorKind>,
    downstream_error: Option<IoErrorKind>,
}

impl DataTransfer {
    fn builder() -> DataTransferBuilder {
        DataTransferBuilder::new()
    }
}

struct DataTransferBuilder {
    result: DataTransferResult,
    upstream_bytes_received: Option<u64>,
    downstream_bytes_sent: Option<u64>,
    upstream_error: Option<ErrorKind>,
    downstream_error: Option<ErrorKind>,
}

impl Default for DataTransferBuilder {
    fn default() -> Self {
        DataTransferBuilder {
            result: DataTransferResult::Succeeded,
            upstream_bytes_received: None,
            downstream_bytes_sent: None,
            upstream_error: None,
            downstream_error: None,
        }
    }
}

impl DataTransferBuilder {
    pub fn new() -> Self {
        DataTransferBuilder::default()
    }

    pub fn result(&mut self, result: DataTransferResult) -> &mut Self {
        self.result = result;
        self
    }

    pub fn upstream_bytes_received(&mut self, bytes: u64) -> &mut Self {
        self.upstream_bytes_received = Some(bytes);
        self
    }

    pub fn downstream_bytes_sent(&mut self, bytes: u64) -> &mut Self {
        self.downstream_bytes_sent = Some(bytes);
        self
    }

    pub fn upstream_error(&mut self, error: ErrorKind) -> &mut Self {
        self.upstream_error = Some(error);
        self.result = Self::error_match(error);
        self
    }

    pub fn downstream_error(&mut self, error: ErrorKind) -> &mut Self {
        self.downstream_error = Some(error);
        self.result = Self::error_match(error);
        self
    }

    fn error_match(err: ErrorKind) -> DataTransferResult {
        match err {
            ErrorKind::ConnectionAborted => DataTransferResult::ConnectionClosed,
            _ => DataTransferResult::Failed,
        }
    }

    pub fn build(&self) -> DataTransfer {
        DataTransfer {
            result: self.result,
            upstream_bytes_received: self.upstream_bytes_received,
            downstream_bytes_sent: self.downstream_bytes_sent,
            upstream_error: self.upstream_error.map(IoErrorKind::ErrorKind),
            downstream_error: self.downstream_error.map(IoErrorKind::ErrorKind),
        }
    }
}

struct FullDuplexPipe<U, D>
where
    U: Readable + Writable,
    D: Readable + Writable,
{
    upstream_pipe: Pipe<ReadHalf<U>, WriteHalf<D>>,
    downstream_pipe: Pipe<ReadHalf<D>, WriteHalf<U>>,
}

fn create_full_duplex_pipe<U, D>(upstream: U, downstream: D) -> FullDuplexPipe<U, D>
where
    U: Readable + Writable,
    D: Readable + Writable,
{
    let (upstream_read, upstream_write) = tokio::io::split(upstream);
    let (downstream_read, downstream_write) = tokio::io::split(downstream);

    FullDuplexPipe {
        upstream_pipe: Pipe {
            reader: upstream_read,
            writer: downstream_write,
        },
        downstream_pipe: Pipe {
            reader: downstream_read,
            writer: upstream_write,
        },
    }
}

pub async fn initiate_full_duplex_data_transfer<S, T>(
    splittable_stream_source: S,
    splittable_stream_target: T,
    tunnel_ttl: Duration,
) -> std::io::Result<DataTransfer>
where
    S: Writable + Readable,
    T: Writable + Readable,
{
    let FullDuplexPipe {
        mut upstream_pipe,
        mut downstream_pipe,
    } = create_full_duplex_pipe(splittable_stream_source, splittable_stream_target);

    // close downstream and upstream pipes after specified duration to be able to provide fairness tp all clients
    let upstream_task_handle =
        tokio::spawn(async move { timeout(tunnel_ttl, upstream_pipe.run()).await });

    let down_stream_handle =
        tokio::spawn(async move { timeout(tunnel_ttl, downstream_pipe.run()).await });

    let join_res = tokio::try_join!(down_stream_handle, upstream_task_handle);

    let mut transfer_result_builder = DataTransfer::builder();

    match join_res {
        Ok((downstream_res_timeout, upstream_res_timeout)) => {
            match upstream_res_timeout {
                Ok(upstream_res) => match upstream_res {
                    Ok(read) => {
                        transfer_result_builder.upstream_bytes_received(read);
                    }
                    Err(err) => {
                        transfer_result_builder.upstream_error(err.kind());
                    }
                },
                Err(_) => {
                    transfer_result_builder.upstream_error(ErrorKind::ConnectionAborted);
                }
            }

            match downstream_res_timeout {
                Ok(downstream_res) => match downstream_res {
                    Ok(read) => {
                        transfer_result_builder.downstream_bytes_sent(read);
                    }
                    Err(err) => {
                        transfer_result_builder.downstream_error(err.kind());
                    }
                },
                Err(_) => {
                    transfer_result_builder.upstream_error(ErrorKind::ConnectionAborted);
                }
            }
        }
        Err(e) => {
            transfer_result_builder.result(if e.is_cancelled() {
                DataTransferResult::Cancelled
            } else {
                DataTransferResult::Panicked
            });
        }
    }
    Ok(transfer_result_builder.build())
}
