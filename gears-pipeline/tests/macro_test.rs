#[cfg(test)]
mod tests {

    #[test]
    fn main() {
        mod pl {
            gears_pipeline::pipeline! {
                vs: {
                    path: "tests/test.glsl"
                    def: [ "FRAGMENT", "VALUE" = "2" ]
                }
                fs: {
                    source: "#version 440\n#include \"include.glsl\""
                    include: "gears-pipeline/tests/"
                }
            }
        }

        // check SPIRV generation
        assert_eq!(1248, pl::VERT_SPIRV.len(), "Vert spirv not what expected");
        assert_eq!(252, pl::FRAG_SPIRV.len(), "Frag spirv not what expected");

        // check UBO struct generation
        pl::UBO { time: 0f32 };
    }
}
