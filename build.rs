use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

fn compile_shader_module(compiler: &mut shaderc::Compiler, kind: shaderc::ShaderKind, path: &Path) {
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_optimization_level(shaderc::OptimizationLevel::Performance);

    let char_id = match kind {
        shaderc::ShaderKind::Vertex => {
            options.add_macro_definition("SHADER_MODULE_VERTEX", None);
            'v'
        }
        shaderc::ShaderKind::Fragment => {
            options.add_macro_definition("SHADER_MODULE_FRAGMENT", None);
            'f'
        }
        _ => 'u',
    };

    let file_stem = path.file_stem().unwrap().to_str().unwrap();
    let cfile = format!("{}.{}", file_stem, char_id);
    let mut file = File::open(path).unwrap();
    let mut source = String::new();
    file.read_to_string(&mut source).unwrap();

    // output spirv
    let spirv = compiler
        .compile_into_spirv(
            source.as_str(),
            kind,
            cfile.as_str(),
            "main",
            Some(&options),
        )
        .unwrap();

    let output_path = format!("{}.spv", cfile);
    let output_path = path.parent().unwrap().join(output_path.as_str());

    let mut file = File::create(output_path).unwrap();
    file.write(spirv.as_binary_u8()).unwrap();

    // output preprocessed glsl for debugging purposes
    let glsl = compiler
        .preprocess(source.as_str(), cfile.as_str(), "main", Some(&options))
        .unwrap();

    let output_path = format!("{}.d.glsl", cfile);
    let output_path = path.parent().unwrap().join(output_path.as_str());

    let mut file = File::create(output_path).unwrap();
    file.write(glsl.as_text().as_bytes()).unwrap();
}

fn main() {
    let mut compiler = shaderc::Compiler::new().unwrap();

    compile_shader_module(
        &mut compiler,
        shaderc::ShaderKind::Vertex,
        &Path::new("src/renderer/shader/default.glsl"),
    );
    compile_shader_module(
        &mut compiler,
        shaderc::ShaderKind::Fragment,
        &Path::new("src/renderer/shader/default.glsl"),
    );

    println!("cargo:rerun-if-changed=src/renderer/shader/default.glsl");
}
