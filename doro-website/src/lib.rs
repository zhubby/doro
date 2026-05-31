use arc_swap::ArcSwap;
use doro_protocol::Website;
use doro_protocol::WebsiteKind;
use doro_protocol::WebsiteStatus;
use pingora::http::RequestHeader;
use pingora::prelude::*;
use pingora::proxy::ProxyHttp;
use pingora::proxy::http_proxy_service;
use pingora::server::Server;
use pingora::upstreams::peer::HttpPeer;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use thiserror::Error;
use url::Url;
use uuid::Uuid;

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

#[derive(Debug, Error)]
pub enum WebsiteRuntimeError {
    #[error("website domain is required")]
    MissingDomain,
    #[error("website upstream URL is required")]
    MissingUpstream,
    #[error("website upstream URL must be an absolute http or https URL")]
    InvalidUpstream,
    #[error("website upstream URL must include a host")]
    MissingUpstreamHost,
    #[error("website upstream URL path, query, and fragment are not supported in v1")]
    UnsupportedUpstreamPath,
    #[error("website listen port must be greater than zero")]
    InvalidListenPort,
    #[error("failed to start website proxy thread")]
    StartThread(#[source] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebsiteRoute {
    pub website_id: Uuid,
    pub primary_domain: String,
    pub aliases: Vec<String>,
    pub listen_port: u16,
    pub upstream_url: String,
    upstream_address: String,
    upstream_host_header: String,
    upstream_sni: String,
    upstream_tls: bool,
}

impl WebsiteRoute {
    pub fn from_website(website: &Website) -> Result<Option<Self>, WebsiteRuntimeError> {
        if website.status != WebsiteStatus::Running {
            return Ok(None);
        }
        if website.kind != WebsiteKind::ReverseProxy {
            return Ok(None);
        }
        if website.listen_port == 0 {
            return Err(WebsiteRuntimeError::InvalidListenPort);
        }

        let primary_domain =
            normalize_domain(&website.primary_domain).ok_or(WebsiteRuntimeError::MissingDomain)?;
        let aliases = website
            .aliases
            .iter()
            .filter_map(|alias| normalize_domain(alias))
            .collect::<Vec<_>>();
        let upstream = parse_upstream(&website.upstream.url)?;

        Ok(Some(Self {
            website_id: website.id,
            primary_domain,
            aliases,
            listen_port: website.listen_port,
            upstream_url: website.upstream.url.clone(),
            upstream_address: upstream.address,
            upstream_host_header: upstream.host_header,
            upstream_sni: upstream.sni,
            upstream_tls: upstream.tls,
        }))
    }

    pub fn upstream_host_header(&self) -> &str {
        &self.upstream_host_header
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebsiteRouteTable {
    routes: HashMap<(String, u16), WebsiteRoute>,
}

impl WebsiteRouteTable {
    pub fn from_websites(websites: &[Website]) -> Result<Self, WebsiteRuntimeError> {
        let mut table = Self::default();
        for website in websites {
            if let Some(route) = WebsiteRoute::from_website(website)? {
                table.insert(route);
            }
        }
        Ok(table)
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn route_for_host(&self, host_header: &str) -> Option<WebsiteRoute> {
        let (host, port) = normalize_host_header(host_header)?;
        if let Some(port) = port
            && let Some(route) = self.routes.get(&(host.clone(), port))
        {
            return Some(route.clone());
        }
        self.routes.get(&(host, 80)).cloned()
    }

    fn insert(&mut self, route: WebsiteRoute) {
        self.routes.insert(
            (route.primary_domain.clone(), route.listen_port),
            route.clone(),
        );
        for alias in &route.aliases {
            self.routes
                .insert((alias.clone(), route.listen_port), route.clone());
        }
    }
}

#[derive(Debug, Clone)]
pub struct WebsiteRuntimeHandle {
    routes: Arc<ArcSwap<WebsiteRouteTable>>,
}

impl Default for WebsiteRuntimeHandle {
    fn default() -> Self {
        Self {
            routes: Arc::new(ArcSwap::from_pointee(WebsiteRouteTable::default())),
        }
    }
}

impl WebsiteRuntimeHandle {
    pub fn reload(&self, websites: &[Website]) -> Result<usize, WebsiteRuntimeError> {
        let table = WebsiteRouteTable::from_websites(websites)?;
        let len = table.len();
        self.routes.store(Arc::new(table));
        Ok(len)
    }

    pub fn route_for_host(&self, host_header: &str) -> Option<WebsiteRoute> {
        self.routes.load().route_for_host(host_header)
    }

    pub fn route_count(&self) -> usize {
        self.routes.load().len()
    }
}

#[derive(Debug)]
pub struct WebsiteRuntime {
    handle: WebsiteRuntimeHandle,
    config: WebsiteRuntimeConfig,
}

impl WebsiteRuntime {
    pub fn new(config: WebsiteRuntimeConfig) -> Self {
        Self {
            handle: WebsiteRuntimeHandle::default(),
            config,
        }
    }

    pub fn handle(&self) -> WebsiteRuntimeHandle {
        self.handle.clone()
    }

    pub fn start(self) -> Result<Option<thread::JoinHandle<()>>, WebsiteRuntimeError> {
        if !self.config.enabled {
            return Ok(None);
        }
        let bind = self.config.http_bind.clone();
        let handle = self.handle.clone();
        let join = thread::Builder::new()
            .name("doro-website-pingora".to_string())
            .spawn(move || {
                if let Err(error) = run_pingora_proxy(bind, handle) {
                    tracing::error!(%error, "website proxy stopped");
                }
            })
            .map_err(WebsiteRuntimeError::StartThread)?;
        Ok(Some(join))
    }
}

#[derive(Clone)]
struct WebsiteProxy {
    routes: WebsiteRuntimeHandle,
}

#[derive(Debug, Default)]
struct WebsiteProxyContext {
    route: Option<WebsiteRoute>,
}

#[async_trait::async_trait]
impl ProxyHttp for WebsiteProxy {
    type CTX = WebsiteProxyContext;

    fn new_ctx(&self) -> Self::CTX {
        WebsiteProxyContext::default()
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let host = session
            .get_header("host")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        let Some(route) = self.routes.route_for_host(host) else {
            session.respond_error(404).await?;
            return Ok(true);
        };
        ctx.route = Some(route);
        Ok(false)
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        let Some(route) = ctx.route.clone() else {
            session.respond_error(404).await?;
            return Ok(Box::new(HttpPeer::new("127.0.0.1:9", false, String::new())));
        };
        Ok(Box::new(HttpPeer::new(
            route.upstream_address,
            route.upstream_tls,
            route.upstream_sni,
        )))
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        if let Some(route) = &ctx.route {
            let _ = upstream_request.insert_header("host", route.upstream_host_header());
        }
        Ok(())
    }
}

fn run_pingora_proxy(bind: String, handle: WebsiteRuntimeHandle) -> anyhow::Result<()> {
    let mut server = Server::new(None)?;
    server.bootstrap();
    let proxy = WebsiteProxy { routes: handle };
    let mut service = http_proxy_service(&server.configuration, proxy);
    service.add_tcp(&bind);
    server.add_service(service);
    tracing::info!(bind, "doro website pingora proxy listening");
    server.run_forever();
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedUpstream {
    address: String,
    host_header: String,
    sni: String,
    tls: bool,
}

fn parse_upstream(value: &str) -> Result<ParsedUpstream, WebsiteRuntimeError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(WebsiteRuntimeError::MissingUpstream);
    }
    let url = Url::parse(value).map_err(|_| WebsiteRuntimeError::InvalidUpstream)?;
    let tls = match url.scheme() {
        "http" => false,
        "https" => true,
        _ => return Err(WebsiteRuntimeError::InvalidUpstream),
    };
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(WebsiteRuntimeError::UnsupportedUpstreamPath);
    }
    let host = url
        .host_str()
        .ok_or(WebsiteRuntimeError::MissingUpstreamHost)?
        .to_string();
    let port = url
        .port_or_known_default()
        .ok_or(WebsiteRuntimeError::InvalidUpstream)?;
    let default_port = if tls { 443 } else { 80 };
    let host_header = if url.port().is_some() || port != default_port {
        format!("{host}:{port}")
    } else {
        host.clone()
    };
    Ok(ParsedUpstream {
        address: format!("{host}:{port}"),
        host_header,
        sni: host,
        tls,
    })
}

fn normalize_domain(value: &str) -> Option<String> {
    let domain = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() {
        None
    } else {
        Some(domain)
    }
}

fn normalize_host_header(value: &str) -> Option<(String, Option<u16>)> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('[') {
        return None;
    }
    let (host, port) = match value.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') && port.chars().all(|c| c.is_ascii_digit()) => {
            (host, port.parse::<u16>().ok())
        }
        _ => (value, None),
    };
    normalize_domain(host).map(|host| (host, port))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use doro_protocol::WebsiteProtocol;
    use doro_protocol::WebsiteProxyTarget;
    use serde_json::json;

    #[test]
    fn route_table_matches_host_header_case_and_port() {
        let website = website("Example.COM", 8080, WebsiteStatus::Running);
        let table = WebsiteRouteTable::from_websites(&[website])
            .unwrap_or_else(|error| panic!("route table should build: {error}"));

        let route = table
            .route_for_host("example.com:8080")
            .unwrap_or_else(|| panic!("route should match host with port"));
        assert_eq!(route.primary_domain, "example.com");

        let route = table
            .route_for_host("EXAMPLE.COM:8080")
            .unwrap_or_else(|| panic!("route should match case-insensitively"));
        assert_eq!(route.primary_domain, "example.com");
    }

    #[test]
    fn stopped_websites_are_not_routed() {
        let website = website("example.com", 8080, WebsiteStatus::Stopped);
        let table = WebsiteRouteTable::from_websites(&[website])
            .unwrap_or_else(|error| panic!("route table should build: {error}"));

        assert!(table.is_empty());
        assert!(table.route_for_host("example.com:8080").is_none());
    }

    #[test]
    fn upstream_url_must_be_http_or_https_with_host() {
        assert!(parse_upstream("http://127.0.0.1:8787").is_ok());
        assert!(parse_upstream("https://service.local").is_ok());
        assert!(matches!(
            parse_upstream("unix:///tmp/service.sock"),
            Err(WebsiteRuntimeError::InvalidUpstream)
        ));
        assert!(parse_upstream("http://").is_err());
    }

    #[test]
    fn missing_routes_do_not_fall_back_to_an_upstream() {
        let table = WebsiteRouteTable::default();

        assert!(table.route_for_host("unknown.example:8080").is_none());
    }

    fn website(domain: &str, listen_port: u16, status: WebsiteStatus) -> Website {
        Website {
            id: Uuid::new_v4(),
            host_id: None,
            name: domain.to_string(),
            primary_domain: domain.to_string(),
            aliases: vec!["www.example.com".to_string()],
            status,
            kind: WebsiteKind::ReverseProxy,
            protocol: WebsiteProtocol::Http,
            listen_port,
            upstream: WebsiteProxyTarget {
                url: "http://127.0.0.1:8787".to_string(),
            },
            app_install_id: None,
            tls_certificate_id: None,
            config: json!({}),
            notes: None,
            last_runtime_error: None,
            last_checked_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
