#version 460
#if VALUE == 2

#[gears_bindgen(uniform(binding = 0))]
struct UBO {
	float time;
} ubo;
// converts into:
// ```layout(binding = 0)uniform UBO { float time;} ubo;```

#[gears_bindgen(in)]
struct VertexData {
	vec2 pos;
	vec3 col;
} vert_in;
// converts into:
// ```layout(location = 0)in vec2 _vert_in_pos;layout(location = 1)in vec3 _vert_in_col;```
// and renames all vert_in. into _vert_in_

#[gears_gen(out)]
struct {
	vec3 col;
} vert_out;
// converts into:
// ```layout(location = 0)out vec3 _vert_out_col;```
// and renames all vert_out. into _vert_out_



void main() {
	gl_Position = vec4(vert_in.pos, 0.0, 1.0);
	vert_out.col = vert_in.col;
}
#endif