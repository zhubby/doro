use crate::error::WebsiteRuntimeError;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedUpstream {
    pub(crate) address: String,
    pub(crate) host_header: String,
    pub(crate) sni: String,
    pub(crate) tls: bool,
}

pub(crate) fn parse_upstream(value: &str) -> Result<ParsedUpstream, WebsiteRuntimeError> {
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

pub(crate) fn normalize_domain(value: &str) -> Option<String> {
    let domain = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() {
        None
    } else {
        Some(domain)
    }
}

pub(crate) fn normalize_host_header(value: &str) -> Option<(String, Option<u16>)> {
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
}
