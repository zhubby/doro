use thiserror::Error;

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
