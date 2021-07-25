use shaderc::ShaderKind;
use std::{fs::File, io::Read, path::PathBuf};

pub type DefinesInput = Vec<(String, Option<String>)>;

const LIBRARIES: &[(&str, &str)] = &[("rand", include_str!("../res/rand.glsl"))];

pub fn compile_shader_module(
    kind: ShaderKind,
    source: &str,
    name: &str,
    entry: &str,
    include_path: Option<PathBuf>,
    defines: &DefinesInput,
    debug: bool,
) -> Result<shaderc::CompilationArtifact, String> {
    let mut compiler = shaderc::Compiler::new().unwrap();

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
        options.add_macro_definition(define, val.as_ref().map_or(None, |s| Some(s.as_str())));
    }

    let result = if debug {
        compiler
            .preprocess(source, name, entry, Some(&options))
            .map_or_else(|err| Err(format!("{}", err)), |res| Err(res.as_text()))
    } else {
        compiler
            .compile_into_spirv(source, kind, name, entry, Some(&options))
            .or_else(|err| Err(format!("{}", err)))
    };

    result.or_else(|err| {
        let source_with_lines: String = source
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{:-4}: {}\n", i + 1, line))
            .collect();

        Err(format!(
            "\n{}:\n{}\nSource code:\n{}",
            if debug {
                "Preprocessed code"
            } else {
                "Shaderc error"
            },
            err,
            source_with_lines.trim_end()
        ))
    })
}
