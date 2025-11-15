use std::io::Result;

fn main() -> Result<()> {
    // Set PROTOC environment variable to use bundled protoc
    std::env::set_var("PROTOC", protobuf_src::protoc());

    // Compile the protocol buffer definitions
    prost_build::compile_protos(
        &["proto/ET.proto", "proto/ETerminal.proto"],
        &["proto/"],
    )?;

    // Rerun if proto files change
    println!("cargo:rerun-if-changed=proto/ET.proto");
    println!("cargo:rerun-if-changed=proto/ETerminal.proto");

    Ok(())
}
