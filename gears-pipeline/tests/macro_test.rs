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

        // check SPIRV generation
        assert_eq!(240, pl::VERT_SPIRV.len(), "Vert spirv not what expected");
        assert_eq!(888, pl::FRAG_SPIRV.len(), "Frag spirv not what expected");

        // check UBO struct generation
        pl::UBO { time: 0.0 };
    }
}
