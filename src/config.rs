use regex::Regex;
use std::time::Duration;

pub const MAX_HTTP_CONNECT_REQUEST_SIZE: usize = 2048;

#[derive(Debug)]
pub struct ProxyConfig {
    pub site_list: Option<ProxySiteList>,
    pub timeout: ProxyTimeout,
}

#[derive(Debug)]
pub struct ProxyTimeout {
    pub http_connect_handshake_each_step: Duration,
    pub tunnel_ttl: Duration,
}

#[derive(Debug)]
pub struct ProxySiteList {
    regex: Regex,
    operate_as_white_list: bool
}

impl ProxySiteList {
    pub fn new(regex: Regex, operate_as_white_list: bool) -> ProxySiteList {
        ProxySiteList {
            regex,
            operate_as_white_list
        }
    }
    pub fn is_white_list(&self) -> bool {
        self.operate_as_white_list
    }
    pub fn contains(&self, site: &str) -> bool {
        self.regex.is_match(site)
    }
}
