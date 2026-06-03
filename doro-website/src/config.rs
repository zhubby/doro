use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebsiteRuntimeConfig {
    pub enabled: bool,
    pub http_bind: String,
}

impl Default for WebsiteRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            http_bind: "127.0.0.1:8080".to_string(),
        }
    }
}

impl WebsiteRuntimeConfig {
    pub fn http_port(&self) -> Option<u16> {
        self.http_bind
            .parse::<SocketAddr>()
            .ok()
            .map(|address| address.port())
    }
}
