use crate::config::WebsiteRuntimeConfig;
use crate::error::WebsiteRuntimeError;
use crate::proxy::run_pingora_proxy;
use crate::route::WebsiteRoute;
use crate::route::WebsiteRouteTable;
use arc_swap::ArcSwap;
use doro_protocol::Website;
use std::sync::Arc;
use std::thread;

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
