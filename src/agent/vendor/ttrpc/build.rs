fn main() {
    protobuf_codegen_pure::Codegen::new()
        .out_dir("src")
        .inputs(&["src/ttrpc.proto"])
        .include("src")
        .run()
        .expect("Codegen failed.");
}
