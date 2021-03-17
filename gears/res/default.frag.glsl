#version 420
#extension GL_ARB_separate_shader_objects : enable

#[gears_gen(in)]
struct {
	vec3 col;
} frag_in;

#[gears_gen(out)]
struct {
	vec4 col;
} frag_out;



void main() {
	frag_out.col = vec4(frag_in.col, 1.0);
}