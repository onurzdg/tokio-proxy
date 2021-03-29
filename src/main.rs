use tokio::net::TcpListener;

use log::{error, info, warn};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use regex::Regex;

use config::*;
use target_connection_provider::*;

mod async_read_write;
mod config;
mod data_transfer;
mod description;
mod errors;
mod http_codec;
mod request_id;
mod request_processor;
mod target_connection_provider;
mod tunnel;

// TODO: read these from command line
const PORT: u16 = 12345;
const MAX_OPEN_CONNECTIONS: usize = 10000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    log4rs::init_file("config/log4rs.yml", Default::default())?;
    let white_list_regex = Regex::new(r"^([0-9A-Za-z]+\.)?(gfycat|giphy)\.com:443$")?;

    // TODO: read these from a config file
    let config = Arc::new(ProxyConfig {
        white_list: ProxyWhitelist {
            regex: white_list_regex,
        }.into(),
        timeout: ProxyTimeout {
            http_connect_handshake_each_step: Duration::from_secs(5),
            tunnel_ttl: Duration::from_secs(30),
        },
    });

    let server_listener = create_server().await?;
    info!(target: "server-status", "Server started - listening on port {}", server_listener.local_addr().expect("failed to get the local address").port());
    let connection_semaphore = Arc::new(Semaphore::new(MAX_OPEN_CONNECTIONS));

    let server_permit_watchdog = {
        let watchdog_connection_semaphore = Arc::clone(&connection_semaphore);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                log::info!(target: "server-status", "available connection permits {} / {}", watchdog_connection_semaphore.available_permits(), MAX_OPEN_CONNECTIONS);
            }
        })
    };

    let server_accept_loop = async {
        loop {
            // Limit number of open connections to avoid crashing the server, which
            // will mitigate DDoS and help us serve requests capped at specified limit
            let permit = Arc::clone(&connection_semaphore).acquire_owned().await;
            if connection_semaphore.available_permits() == 0 {
                warn!(target: "server-status", "Server is running at capacity!");
            }
            // Wait to receive connections from clients
            let stream_accept_result = server_listener.accept().await;
            let config = Arc::clone(&config);
            match stream_accept_result {
                Ok((stream, _)) => {
                    tokio::spawn(async move {
                        let _permit = permit;
                        let req_res = request_processor::process(
                            stream,
                            DefaultTargetConnectionProvider,
                            config,
                        )
                        .await;
                        match req_res {
                            Ok(res) => {
                                let request_serialization_result = serde_json::to_string(&res);
                                match request_serialization_result {
                                    Ok(res) => info!(target: "request-result", "{}", res),
                                    Err(err) => {
                                        error!(target: "request-result", "RequestResult serialization failed: {:?}", err)
                                    }
                                }
                            },
                            Err(err) => {
                                error!("Error occurred while proxying request {:?}", err);
                            }
                        }
                    });
                },
                Err(err) => {
                    drop(permit);
                    error!("Client failed to establish connection due to {:?}", err);
                }
            }
        }
    };
    let (res, _) = tokio::join!(server_permit_watchdog, server_accept_loop);
    if let Err(err) = res {
        error!(target: "server-status", "{:?}", err);
    }
    Ok(())
}

async fn create_server() -> std::io::Result<TcpListener> {
    TcpListener::bind(format!("127.0.0.1:{}", PORT))
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                error!("Port {} is already being used by another program", PORT);
            }
            e
        })
}
