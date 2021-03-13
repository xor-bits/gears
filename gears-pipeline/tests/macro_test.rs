#[cfg(test)]
mod tests {

    #[test]
    fn main() {
        mod pl {
            gears_pipeline::pipeline! {
                vs: {
                    source: "#version 440\n#include \"include.glsl\""
                    include: "gears-pipeline/tests/"
                }
                fs: {
                    path: "tests/test.glsl"
                    def: [ "FRAGMENT", "VALUE" = "2" ]
                }
            }
        }

        assert_eq!(
            140,
            pl::VERTEX_SPIRV.len(),
            "Vertex spirv not what expected"
        );
        assert_eq!(
            484,
            pl::FRAGMENT_SPIRV.len(),
            "Fragment spirv not what expected"
        );
    }
}
