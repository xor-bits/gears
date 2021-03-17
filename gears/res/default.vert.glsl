#version 420
#extension GL_ARB_separate_shader_objects : enable

#[gears_bindgen(uniform(binding = 0))]
struct UBO {
	mat4 model_matrix;
} ubo;

#[gears_bindgen(in)]
struct VertexData {
	vec2 pos;
	vec3 col;
} vert_in;

#[gears_gen(out)]
struct {
	vec3 col;
} vert_out;



void main() {
	gl_Position = ubo.model_matrix * vec4(vert_in.pos, 0.0, 1.0);
	vert_out.col = vert_in.col;
}