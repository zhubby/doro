use chrono::Utc;
use doro_protocol::{
    Website, WebsiteKind, WebsiteProtocol, WebsiteProxyTarget, WebsiteStatus, grpc,
};
use doro_website::WebsiteRuntimeHandle;
use serde_json::json;
use uuid::Uuid;

pub(crate) fn apply_website_routes(
    runtime: &WebsiteRuntimeHandle,
    routes: Vec<grpc::WebsiteRoute>,
) -> Result<usize, String> {
    let websites = routes
        .into_iter()
        .map(website_from_grpc_route)
        .collect::<Result<Vec<_>, _>>()?;
    runtime.reload(&websites).map_err(|error| error.to_string())
}

fn website_from_grpc_route(route: grpc::WebsiteRoute) -> Result<Website, String> {
    let website_id = Uuid::parse_str(&route.website_id)
        .map_err(|_| "website route website_id must be a uuid".to_string())?;
    let status = parse_website_status(&route.status)?;
    let kind = parse_website_kind(&route.kind)?;
    let protocol = parse_website_protocol(&route.protocol)?;
    if kind != WebsiteKind::ReverseProxy || protocol != WebsiteProtocol::Http {
        return Err(
            "agent website runtime currently supports only HTTP reverse proxy routes".to_string(),
        );
    }
    let listen_port =
        u16::try_from(route.listen_port).map_err(|_| "website listen port is invalid")?;
    let config = serde_json::from_str(&route.config_json).unwrap_or_else(|_| {
        json!({
            "raw": route.config_json
        })
    });
    Ok(Website {
        id: website_id,
        host_id: Some(Uuid::nil()),
        name: route.primary_domain.clone(),
        primary_domain: route.primary_domain,
        aliases: route.aliases,
        status,
        kind,
        protocol,
        listen_port,
        upstream: WebsiteProxyTarget {
            url: route.upstream_url,
        },
        app_install_id: None,
        tls_certificate_id: None,
        config,
        notes: None,
        last_runtime_error: None,
        last_checked_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    })
}

fn parse_website_status(value: &str) -> Result<WebsiteStatus, String> {
    match normalize_enum_token(value).as_str() {
        "running" => Ok(WebsiteStatus::Running),
        "stopped" => Ok(WebsiteStatus::Stopped),
        "warning" => Ok(WebsiteStatus::Warning),
        _ => Err("website route status is invalid".to_string()),
    }
}

fn parse_website_kind(value: &str) -> Result<WebsiteKind, String> {
    match normalize_enum_token(value).as_str() {
        "reverse_proxy" => Ok(WebsiteKind::ReverseProxy),
        "static_site" => Ok(WebsiteKind::StaticSite),
        "tcp_proxy" => Ok(WebsiteKind::TcpProxy),
        "udp_proxy" => Ok(WebsiteKind::UdpProxy),
        _ => Err("website route kind is invalid".to_string()),
    }
}

fn parse_website_protocol(value: &str) -> Result<WebsiteProtocol, String> {
    match normalize_enum_token(value).as_str() {
        "http" => Ok(WebsiteProtocol::Http),
        "https" => Ok(WebsiteProtocol::Https),
        "tcp" => Ok(WebsiteProtocol::Tcp),
        "udp" => Ok(WebsiteProtocol::Udp),
        _ => Err("website route protocol is invalid".to_string()),
    }
}

fn normalize_enum_token(value: &str) -> String {
    let mut token = String::new();
    for (index, character) in value.chars().enumerate() {
        if character == '-' || character == ' ' {
            token.push('_');
        } else if character.is_uppercase() {
            if index > 0 {
                token.push('_');
            }
            token.extend(character.to_lowercase());
        } else {
            token.push(character);
        }
    }
    token
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_website_routes_do_not_replace_previous_route_table() {
        let runtime = WebsiteRuntimeHandle::default();
        let mut route = grpc::WebsiteRoute {
            website_id: Uuid::new_v4().to_string(),
            primary_domain: "example.com".to_string(),
            aliases: Vec::new(),
            status: "running".to_string(),
            kind: "reverse_proxy".to_string(),
            protocol: "http".to_string(),
            listen_port: 8080,
            upstream_url: "http://127.0.0.1:8787".to_string(),
            config_json: "{}".to_string(),
        };

        let count = apply_website_routes(&runtime, vec![route.clone()])
            .unwrap_or_else(|error| panic!("valid website route should apply: {error}"));
        assert_eq!(count, 1);
        assert_eq!(runtime.route_count(), 1);
        assert!(runtime.route_for_host("example.com:8080").is_some());

        route.website_id = "not-a-uuid".to_string();
        let error = match apply_website_routes(&runtime, vec![route]) {
            Ok(_) => panic!("invalid website route should fail"),
            Err(error) => error,
        };
        assert!(error.contains("website_id"));
        assert_eq!(runtime.route_count(), 1);
        assert!(runtime.route_for_host("example.com:8080").is_some());
    }
}
