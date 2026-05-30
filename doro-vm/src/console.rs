use crate::VmConsoleEndpoint;

pub fn vnc_endpoint(host: impl Into<String>, port: u16) -> VmConsoleEndpoint {
    VmConsoleEndpoint {
        kind: "vnc".to_string(),
        host: host.into(),
        port,
        path: None,
        token: None,
    }
}
