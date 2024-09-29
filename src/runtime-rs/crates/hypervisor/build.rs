use ttrpc_codegen::{Codegen, Customize,ProtobufCustomize};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protobuf_customized = ProtobufCustomize::default().gen_mod_rs(false);

    Codegen::new()
        .out_dir("src/remote/protocols")
        .inputs([
            "src/remote/protocols/hypervisor.proto",
        ])
        .include("src/remote/protocols")
        .rust_protobuf()
        .customize(Customize {
            async_all: true,
            ..Default::default()
        })
        .rust_protobuf_customize(protobuf_customized)
        .run()
        .expect("Gen async code failed.");
    Ok(())
}
