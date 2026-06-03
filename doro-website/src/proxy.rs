use crate::route::WebsiteRoute;
use crate::runtime::WebsiteRuntimeHandle;
use pingora::http::RequestHeader;
use pingora::prelude::*;
use pingora::proxy::ProxyHttp;
use pingora::proxy::http_proxy_service;
use pingora::server::Server;
use pingora::upstreams::peer::HttpPeer;

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
            route.upstream_address(),
            route.upstream_tls(),
            route.upstream_sni().to_string(),
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

pub(crate) fn run_pingora_proxy(bind: String, handle: WebsiteRuntimeHandle) -> anyhow::Result<()> {
    let mut server = Server::new(None)?;
    server.bootstrap();
    let proxy = WebsiteProxy { routes: handle };
    let mut service = http_proxy_service(&server.configuration, proxy);
    service.add_tcp(&bind);
    server.add_service(service);
    tracing::info!(bind, "doro website pingora proxy listening");
    server.run_forever();
}
