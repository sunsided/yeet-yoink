fn main() {
    let proto_includes = ["proto/"];

    let mut config = prost_build::Config::new();
    config.protoc_arg("--experimental_allow_proto3_optional");

    config
        .compile_protos(&["proto/metadata.proto"], &proto_includes)
        .expect("Failed to compile protocol buffers");
}
