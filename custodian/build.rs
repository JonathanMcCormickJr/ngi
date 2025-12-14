fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/custodian.proto", "../db/proto/db.proto", "../admin/proto/admin.proto"], &["proto", "../db/proto", "../admin/proto"])?;
    Ok(())
}