use crate::async_read_write::{Readable, Writable};
use crate::config::ProxyConfig;
use crate::errors::{HttpTunnelRequestDecodeError, HttpTunnelRequestError};
use crate::http_codec::{HttpCodec, HttpTunnelRequestResult, HttpTunnelTarget};
use crate::request_id::RequestId;
use crate::target_connection_provider::TargetConnectionProvider;
use futures::stream::SplitStream;
use futures::{SinkExt, StreamExt};
use log::{error, info};
use tokio::time::timeout;
use tokio_util::codec::{Decoder, Encoder, Framed};

pub struct Tunnel<U, D>
where
    U: Readable + Writable,
    D: Readable + Writable,
{
    source: U,
    target: D,
}

impl<U, D> Tunnel<U, D>
where
    U: Readable + Writable,
    D: Readable + Writable,
{
    pub fn source_and_target(self) -> (U, D) {
        (self.source, self.target)
    }
}

pub async fn create_tunnel<S, P>(
    stream: S,
    target_connection_provider: P,
    config: &ProxyConfig,
    id: &RequestId,
) -> (
    Result<Tunnel<S, P::ReadableWritable>, HttpTunnelRequestError>,
    Option<HttpTunnelTarget>,
)
where
    S: Readable + Writable + Unpin, // Unpin is necessary to be able to reunite client/source stream
    P: TargetConnectionProvider,
{
    let (mut write_sink, mut read_stream) = Framed::new(stream, HttpCodec).split();
    let (tunnel_request_result, target_address) =
        process_tunnel_request(&mut read_stream, target_connection_provider, config, id).await;

    let request_result = match tunnel_request_result {
        Ok(_) => HttpTunnelRequestResult::Success,
        Err(ref err) => HttpTunnelRequestResult::Error(err.clone()),
    };
    // relay response to the client
    let response_relayed_result_with_timeout = timeout(
        config.timeout.http_connect_handshake_each_step,
        write_sink.send(request_result.clone()),
    )
    .await;

    match response_relayed_result_with_timeout {
        Ok(response_relayed_result) => {
            match response_relayed_result {
                Ok(_) => {
                    match tunnel_request_result {
                        Ok(target_stream) => {
                            // reunite original stream parts
                            match write_sink.reunite(read_stream) {
                                Ok(framed_union) => {
                                    let original_client_stream = framed_union.into_inner();
                                    if let Some(ref target) = target_address {
                                        info!(target: "tunnel-established", "Established tunnel to {} {}", target, id);
                                    }
                                    (
                                        Ok(Tunnel {
                                            source: original_client_stream,
                                            target: target_stream,
                                        }),
                                        target_address,
                                    )
                                }
                                Err(err) => {
                                    error!(target: "stream-reunite-failed", "Failed to reunite original stream due to {:?} {}", err, id);
                                    (Err(HttpTunnelRequestError::InternalError), target_address)
                                }
                            }
                        }
                        Err(err) => (Err(err), target_address),
                    }
                }
                Err(err) => {
                    error!(target: "response-relay-error", "Could not relay the response to the client due to {:?} {}.", err, id);
                    (Err(HttpTunnelRequestError::BadGateway), target_address)
                }
            }
        }
        Err(_) => {
            error!(target: "response-relay-timeout", "Could not relay the response to the client within {:?}. {}", config.timeout.http_connect_handshake_each_step, id);
            (Err(HttpTunnelRequestError::RequestTimeout), target_address)
        }
    }
}

async fn process_tunnel_request<S, C, P>(
    read_stream: &mut SplitStream<Framed<S, C>>,
    target_connection_provider: P,
    config: &ProxyConfig,
    id: &RequestId,
) -> (
    Result<P::ReadableWritable, HttpTunnelRequestError>,
    Option<HttpTunnelTarget>,
)
where
    S: Readable + Writable,
    C: Decoder<Error = HttpTunnelRequestDecodeError, Item = HttpTunnelTarget>
        + Encoder<HttpTunnelRequestResult>,
    P: TargetConnectionProvider,
{
    let decoded_request_result_with_timeout = timeout(
        config.timeout.http_connect_handshake_each_step,
        read_stream.next(),
    )
    .await;
    use HttpTunnelRequestError::*;
    match decoded_request_result_with_timeout {
        Ok(decoded_request_result) => match decoded_request_result {
            Some(Ok(target_address)) => {
                if let Some(ref white_list) = config.white_list {
                    if !white_list.contains(target_address.target()) {
                        error!(target: "forbidden-target", "Rejected routing for {} as it is not in the whitelist. {}", target_address, id);
                        return (Err(Forbidden), target_address.into());
                    }
                }

                let connect_result_with_timeout = target_connection_provider
                    .connect(
                        target_address.target(),
                        config.timeout.http_connect_handshake_each_step,
                    )
                    .await;
                match connect_result_with_timeout {
                    Ok(tcp_stream) => (Ok(tcp_stream), target_address.into()),
                    Err(err) => {
                        error!(target: "failed-to-connect-to-target", "Failed to connect to target {} due to {:?}. {}",  target_address, err, id);
                        match err.kind() {
                            std::io::ErrorKind::TimedOut => {
                                (Err(GatewayTimeout), target_address.into())
                            }
                            _ => (Err(BadGateway), target_address.into()),
                        }
                    }
                }
            }
            Some(Err(decode_error)) => {
                error!(target: "bad-request", "Bad client request: {:?}. {}", decode_error, id);
                (Err(RequestDecodeError(decode_error)), None)
            }
            None => {
                error!(target: "incomplete-request", "Request is incomplete. {}", id);
                (Err(BadRequest), None)
            }
        },
        Err(_) => {
            error!(target: "request-timeout", "Could not send HTTP CONNECT request within {:?} {}", config.timeout.http_connect_handshake_each_step, id);
            (Err(RequestTimeout), None)
        }
    }
}
