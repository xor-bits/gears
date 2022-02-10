#version 420

layout(location = 0) in vec3 vi_pos;
layout(location = 1) in float vi_exp;

layout(location = 0) out float fi_exp;

layout(binding = 0) uniform UBO {
	mat4 mvp;
} ubo;



void main() {
	gl_Position = ubo.mvp * vec4(vi_pos, 1.0);
	fi_exp = vi_exp;
}