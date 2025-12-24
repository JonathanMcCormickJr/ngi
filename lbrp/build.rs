fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: This is safe because build scripts are single-threaded and run before
    // any other code. We're setting PROTOC once at the start of the build process.
    unsafe {
        std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);
    }

    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(
            &[
                "../custodian/proto/custodian.proto",
                "../db/proto/db.proto",
                "../auth/proto/auth.proto",
                "../admin/proto/admin.proto",
            ],
            &[
                "../custodian/proto",
                "../db/proto",
                "../auth/proto",
                "../admin/proto",
            ],
        )?;
    Ok(())
}
