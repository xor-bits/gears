use std::{fs::File, io::Read, path::Path};

use proc_macro2::Punct;
use syn::{parse::ParseStream, Error, LitStr, Token};

// struct/enum

pub struct DefinesInput {
    defines: Vec<(String, Option<String>)>,
}

// impl

impl DefinesInput {
    pub fn new() -> DefinesInput {
        DefinesInput {
            defines: Vec::new(),
        }
    }
}

// trait impl

impl std::ops::AddAssign for DefinesInput {
    fn add_assign(&mut self, rhs: Self) {
        let mut defines = rhs.defines;
        self.defines.append(&mut defines);
    }
}

impl syn::parse::Parse for DefinesInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut defines = Vec::new();

        while !input.is_empty() {
            let name: LitStr = input.parse()?;

            if input.is_empty() {
                defines.push((name.value(), None));
                break;
            }

            let punct: Punct = input.parse()?;
            match punct.as_char() {
                '=' => {
                    let value: LitStr = input.parse()?;
                    defines.push((name.value(), Some(value.value())));

                    if input.is_empty() {
                        break;
                    }

                    input.parse::<Token![,]>()?;
                }
                ',' => {
                    continue;
                }
                _ => {
                    return Err(Error::new(
                        punct.span(),
                        "Invalid punctuation, only '=' and ',' are valid",
                    ))
                }
            }
        }

        Ok(Self { defines })
    }
}

// pub fn

pub fn compile_shader_module(
    kind: shaderc::ShaderKind,
    source: &str,
    name: &str,
    entry: &str,
    include_path: Option<&Path>,
    defines: &DefinesInput,
    default_defines: bool,
    debug: bool,
) -> Result<shaderc::CompilationArtifact, String> {
    let compiler = unsafe {
        if STATIC_COMPILER.is_none() {
            STATIC_COMPILER = Some(
                shaderc::Compiler::new()
                    .unwrap_or_else(|| panic!("Could not create a shaderc Compiler")),
            );
            STATIC_COMPILER.as_mut().unwrap()
        } else {
            STATIC_COMPILER.as_mut().unwrap()
        }
    };

    let mut options = shaderc::CompileOptions::new()
        .unwrap_or_else(|| panic!("Could not create a shaderc CompileOptions"));
    options.set_optimization_level(shaderc::OptimizationLevel::Zero);
    options.set_include_callback(
        |name: &str, _include_type: shaderc::IncludeType, _source: &str, _depth: usize| {
            let full_path = include_path.ok_or("No include path")?.join(name);
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

    match (kind, default_defines) {
        (shaderc::ShaderKind::Vertex, true) => {
            options.add_macro_definition("GEARS_VERTEX", None);
            options.add_macro_definition(
                "GEARS_IN(_location, _data)",
                Some("layout(location = _location) in _data;"),
            );
            options.add_macro_definition(
                "GEARS_INOUT(_location, _data)",
                Some("layout(location = _location) out _data;"),
            );
            options.add_macro_definition(
                "GEARS_VERT_UBO(_location, _data)",
                Some("layout(binding = _location) _data;"),
            );
        }
        (shaderc::ShaderKind::Fragment, true) => {
            options.add_macro_definition("GEARS_FRAGMENT", None);
            options.add_macro_definition(
                "GEARS_OUT(_location, _data)",
                Some("layout(location = _location) out _data;"),
            );
            options.add_macro_definition(
                "GEARS_INOUT(_location, _data)",
                Some("layout(location = _location) in _data;"),
            );
        }
        _ => (),
    };

    for (define, val) in defines.defines.iter() {
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
            "Error:\n{}\nSource:\n{}",
            err,
            source_with_lines.trim_end()
        ))
    })
}

static mut STATIC_COMPILER: Option<shaderc::Compiler> = None;
