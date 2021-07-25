#version 420

layout(location = 0) in vec3 in_position;
layout(location = 1) in float in_exposure;

layout(location = 0) out float out_exposure;

layout(binding = 0) uniform UBO {
	mat4 mvp;
} ubo;



void main() {
	gl_Position = ubo.mvp * vec4(in_position, 1.0);
	out_exposure = in_exposure;
}