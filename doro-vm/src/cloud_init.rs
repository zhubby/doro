use serde_json::Value;

pub fn user_data(metadata: &Value) -> String {
    let hostname = metadata
        .get("hostname")
        .and_then(Value::as_str)
        .unwrap_or("doro-vm");
    format!("#cloud-config\nhostname: {hostname}\n")
}
