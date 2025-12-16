fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .compile_protos(
            &[
                "../admin/proto/admin.proto",
                "../auth/proto/auth.proto",
                "../custodian/proto/custodian.proto",
            ],
            &["../admin/proto", "../auth/proto", "../custodian/proto", "../shared/proto"],
        )?;
    Ok(())
}
