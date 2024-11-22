fn main() {
    tonic_build::compile_protos("proto/send_user_data.proto")
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
