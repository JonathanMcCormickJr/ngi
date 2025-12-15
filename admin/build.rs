fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use `tonic_prost_build` to compile protos for admin service
    tonic_prost_build::compile_protos("proto/admin.proto")?;
    Ok(())
}
