use regex::Regex;
use std::time::Duration;

pub const MAX_HTTP_CONNECT_REQUEST_SIZE: usize = 2048;

#[derive(Debug)]
pub struct ProxyConfig {
    pub white_list: Option<ProxyWhitelist>,
    pub timeout: ProxyTimeout,
}

#[derive(Debug)]
pub struct ProxyTimeout {
    pub http_connect_handshake_each_step: Duration,
    pub tunnel_ttl: Duration,
}

#[derive(Debug)]
pub struct ProxyWhitelist {
    pub regex: Regex,
}

impl ProxyWhitelist {
    pub fn contains(&self, site: &str) -> bool {
        self.regex.is_match(site)
    }
}
