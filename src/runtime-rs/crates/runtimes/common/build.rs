use protobuf_codegen::Codegen;

fn main() {
    Codegen::new()
        .pure()
        .cargo_out_dir("cri")
        .input("src/protos/api.proto")
        .input("src/protos/github.com/gogo/protobuf/gogoproto/gogo.proto")
        .include("src/protos")
        .run_from_script();
}
