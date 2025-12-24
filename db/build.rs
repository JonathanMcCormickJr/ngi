fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: This is safe because build scripts are single-threaded and run before
    // any other code. We're setting PROTOC once at the start of the build process.
    unsafe {
        std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);
    }

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/db.proto"], &["proto"])?;
    Ok(())
}
