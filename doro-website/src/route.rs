use crate::error::WebsiteRuntimeError;
use crate::upstream::normalize_domain;
use crate::upstream::normalize_host_header;
use crate::upstream::parse_upstream;
use doro_protocol::Website;
use doro_protocol::WebsiteKind;
use doro_protocol::WebsiteStatus;
use std::collections::HashMap;
use uuid::Uuid;

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

    pub(crate) fn upstream_address(&self) -> &str {
        &self.upstream_address
    }

    pub(crate) fn upstream_sni(&self) -> &str {
        &self.upstream_sni
    }

    pub(crate) fn upstream_tls(&self) -> bool {
        self.upstream_tls
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
