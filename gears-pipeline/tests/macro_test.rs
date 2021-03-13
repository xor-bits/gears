#[cfg(test)]
mod tests {
    use gears_pipeline::pipeline;

    #[test]
    fn main() {
        pipeline! {
            macro_pipeline
            vs: {
                source: "#version 440\n#include \"include.glsl\""
                include: "gears-pipeline/tests/"
            }
            fs: {
                path: "tests/test.glsl"
            }
        };

        assert_eq!(
            140,
            macro_pipeline::VERTEX_SPIRV.len(),
            "Vertex spirv not what expected"
        );
        assert_eq!(
            484,
            macro_pipeline::FRAGMENT_SPIRV.len(),
            "Fragment spirv not what expected"
        );
    }
}
