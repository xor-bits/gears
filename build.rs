use std::process::Command;

fn main() {
    Command::new("./src/renderer/shader/compile.sh")
        .status()
        .unwrap();

    println!("cargo:rerun-if-changed=res/vert.glsl");
    println!("cargo:rerun-if-changed=res/frag.glsl");
}
