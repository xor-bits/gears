#version 420
#extension GL_ARB_separate_shader_objects : enable

#[gears_bindgen(uniform)]
struct UBO {
	mat4 mvp;
} ubo;

#[gears_bindgen(in)]
struct VertexData {
	/* uint raw_data; */
	vec3 position;
	float exposure;
} vert_in;

#[gears_gen(out)]
struct VFSharedData {
	float exposure;
} vert_out;



void main() {
	/* vec3 position = vec3(
		float((vert_in.raw_data & 0x00007F) >> 0),
		float((vert_in.raw_data & 0x003F80) >> 7),
		float((vert_in.raw_data & 0x1FC000) >> 14));

	float exposure = float((vert_in.raw_data & 0x600000) >> 21) * 0.25 + 0.25;

	gl_Position = ubo.mvp * vec4(position, 1.0);
	vert_out.exposure = exposure; */

	gl_Position = ubo.mvp * vec4(vert_in.position, 1.0);

#if defined(DEBUGGING)
	vert_out.exposure = (gl_Position.z) * 0.002;
#else
	vert_out.exposure = vert_in.exposure;
#endif
}