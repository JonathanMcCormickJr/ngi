fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &["proto/auth.proto", "../db/proto/db.proto"],
            &["proto", "../db/proto"]
        )?;
    Ok(())
}