use tokio::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};

pub trait Readable: AsyncRead + Send + 'static {}
pub trait Writable: AsyncWrite + Send + 'static {}

// general implementations
impl<T: AsyncRead + Send + 'static> Readable for T {}
impl<T: AsyncWrite + Send + 'static> Writable for T {}

pub struct Pipe<R, W>
where
    R: Readable,
    W: Writable,
{
    pub reader: R,
    pub writer: W,
}

impl<S, D> Pipe<ReadHalf<S>, WriteHalf<D>>
where
    S: Readable + Writable,
    D: Readable + Writable,
{
    pub async fn run(&mut self) -> std::io::Result<u64> {
        tokio::io::copy(&mut self.reader, &mut self.writer).await
    }
}
