fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure().compile_protos(&["proto/doro/agent/v1/agent.proto"], &["proto"])?;
    Ok(())
}
