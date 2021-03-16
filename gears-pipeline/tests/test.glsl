#version 460
#if VALUE == 2

#[gears_bindgen(uniform(binding = 0))]
struct UBO {
	float time;
} ubo;
// into 
/* layout(binding = 0) uniform UBO {
	float time;
} ubo; */

#[gears_bindgen(in)]
struct VertexData {
	vec2 pos;
	vec3 col;
} vert_in;
// into 
/* layout(location = 0) in vec2 _vert_in_pos;
layout(location = 1) in vec3 _vert_in_col; */

layout(location = 0) out vec3 frag_color;

void main() {
	float x = sin(4.0);
	x = abs(x);

	if (x > 0.5) {
		x *= 2;
	}

	frag_color = vec3(x, 1.0, 0.5);
}
#endif