use std::{
    env,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=PROTOC_INCLUDE");

    let protos = [PathBuf::from("proto/doro/agent/v1/agent.proto")];
    let mut includes = vec![PathBuf::from("proto")];

    if let Some(protoc_include) = env::var_os("PROTOC_INCLUDE") {
        includes.push(PathBuf::from(protoc_include));
    }

    for include in [
        "/usr/include",
        "/usr/local/include",
        "/opt/homebrew/include",
        "/opt/local/include",
    ] {
        if Path::new(include)
            .join("google/protobuf/timestamp.proto")
            .exists()
        {
            includes.push(PathBuf::from(include));
        }
    }

    tonic_build::configure().compile_protos(&protos, &includes)?;
    Ok(())
}
