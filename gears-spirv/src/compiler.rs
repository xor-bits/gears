use shaderc::ShaderKind;
use std::{fs::File, io::Read, path::PathBuf};

pub type DefinesInput = Vec<(String, Option<String>)>;

const LIBRARIES: &[(&str, &str)] = &[("rand", include_str!("../res/rand.glsl"))];

fn compiler() -> shaderc::Compiler {
    shaderc::Compiler::new().unwrap()
}

fn options(include_path: Option<PathBuf>, defines: &DefinesInput) -> shaderc::CompileOptions<'_> {
    let mut options = shaderc::CompileOptions::new()
        .unwrap_or_else(|| panic!("Could not create a shaderc CompileOptions"));
    options.set_optimization_level(shaderc::OptimizationLevel::Zero);

    if let Some(include_path) = include_path {
        options.set_include_callback(
            move |name: &str, _include_type: shaderc::IncludeType, _source: &str, _depth: usize| {
                // include built in library
                for (lib, lib_content) in LIBRARIES.iter() {
                    if name == *lib {
                        return Ok(shaderc::ResolvedInclude {
                            content: String::from(*lib_content),
                            resolved_name: String::from(*lib),
                        });
                    }
                }

                // include from path
                let full_path = include_path.join(name);
                let mut file = File::open(&full_path).or(Err(format!(
                    "Could not open file '{}'",
                    full_path.to_str().ok_or("Path unwrap failed")?
                )))?;

                let mut content = String::new();
                file.read_to_string(&mut content).or(Err(format!(
                    "Could not read from file '{}'",
                    full_path.to_str().ok_or("Path unwrap failed")?
                )))?;

                Ok(shaderc::ResolvedInclude {
                    content,
                    resolved_name: String::from(
                        full_path
                            .to_str()
                            .unwrap_or_else(|| panic!("Path unwrap failed")),
                    ),
                })
            },
        );
    }

    for (define, val) in defines.iter() {
        options.add_macro_definition(define, val.as_ref().map(|s| s.as_str()));
    }

    options
}

pub fn preprocess_shader_module(
    source: &str,
    name: &str,
    entry: &str,
    include_path: Option<PathBuf>,
    defines: &DefinesInput,
) -> Result<String, String> {
    let mut compiler = compiler();
    let options = options(include_path, defines);

    let result = compiler
        .preprocess(source, name, entry, Some(&options))
        .map_err(|err| err.to_string())?;

    Ok(result.as_text())
}

pub fn compile_shader_module(
    kind: ShaderKind,
    source: &str,
    name: &str,
    entry: &str,
    include_path: Option<PathBuf>,
    defines: &DefinesInput,
) -> Result<shaderc::CompilationArtifact, String> {
    let mut compiler = compiler();
    let options = options(include_path, defines);

    let result = compiler
        .compile_into_spirv(source, kind, name, entry, Some(&options))
        .map_err(|err| err.to_string())?;
    Ok(result)
}
