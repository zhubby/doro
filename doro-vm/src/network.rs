use crate::VmNetworkMode;
use crate::VmNetworkSpec;
use crate::VmProviderError;

#[derive(Debug, Clone)]
pub struct NetworkPolicy {
    pub user_nat_enabled: bool,
    pub allowed_bridges: Vec<String>,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            user_nat_enabled: true,
            allowed_bridges: Vec::new(),
        }
    }
}

impl NetworkPolicy {
    pub fn validate(&self, network: &VmNetworkSpec) -> Result<(), VmProviderError> {
        match network.mode {
            VmNetworkMode::UserNat if self.user_nat_enabled => Ok(()),
            VmNetworkMode::UserNat => Err(VmProviderError::InvalidRequest(
                "user NAT networking is disabled".to_string(),
            )),
            VmNetworkMode::BridgeTap => {
                let Some(bridge) = network.bridge.as_deref() else {
                    return Err(VmProviderError::InvalidRequest(
                        "bridge networking requires a bridge name".to_string(),
                    ));
                };
                if self.allowed_bridges.iter().any(|allowed| allowed == bridge) {
                    Ok(())
                } else {
                    Err(VmProviderError::InvalidRequest(format!(
                        "bridge {bridge} is not allowed"
                    )))
                }
            }
        }
    }
}
