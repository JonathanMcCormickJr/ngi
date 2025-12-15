fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(&[
            "../custodian/proto/custodian.proto",
            "../db/proto/db.proto",
        ], &[
            "../custodian/proto",
            "../db/proto",
        ])?;
    Ok(())
}