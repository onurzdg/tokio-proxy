use crate::async_read_write::{Readable, Writable};
use crate::config::ProxyConfig;
use crate::data_transfer::{initiate_full_duplex_data_transfer, DataTransfer};
use crate::errors::HttpTunnelRequestError;
use crate::request_id::RequestId;
use crate::target_connection_provider::TargetConnectionProvider;
use crate::tunnel::create_tunnel;
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub async fn process<T, P>(
    stream: T,
    target_connection_provider: P,
    config: Arc<ProxyConfig>,
) -> std::io::Result<RequestResult>
where
    T: Readable + Writable + Unpin,
    P: TargetConnectionProvider,
{
    let request_id = RequestId::generate();
    let start_time = Instant::now();
    let (tunnel_creation_result, target_address) =
        create_tunnel(stream, target_connection_provider, &config, &request_id).await;
    let target_address = target_address.map(|t| t.target().to_string());

    match tunnel_creation_result {
        Ok(tunnel) => {
            let (source, target) = tunnel.source_and_target();
            let result =
                initiate_full_duplex_data_transfer(source, target, config.timeout.tunnel_ttl).await;
            result.map(|res| RequestResult {
                id: request_id.id().to_string(),
                tunnel_request_error: None,
                data_transfer: Some(res),
                duration: Instant::now().duration_since(start_time),
                target_address,
            })
        }
        Err(err) => Ok(RequestResult {
            id: request_id.id().to_string(),
            tunnel_request_error: Some(err),
            data_transfer: None,
            duration: Instant::now().duration_since(start_time),
            target_address,
        }),
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct RequestResult {
    id: String,
    data_transfer: Option<DataTransfer>,
    tunnel_request_error: Option<HttpTunnelRequestError>,
    duration: Duration,
    target_address: Option<String>,
}
