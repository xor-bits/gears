#version 420
#extension GL_ARB_separate_shader_objects : enable

#[gears_gen(in)]
struct VFSharedData {
	float exposure;
} frag_in;

#[gears_gen(out)]
struct VFFragmentData {
	vec4 col;
} frag_out;



void main() {
#if defined(DEBUGGING)
	frag_out.col = vec4(1.0, 0.0, 0.0, 1.0);
#else
	frag_out.col = vec4(frag_in.exposure, frag_in.exposure, frag_in.exposure, 1.0);
#endif
}