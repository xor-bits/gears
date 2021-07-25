#version 420

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 norm;

layout(location = 0) out float exposure;

layout(binding = 0) uniform UBO {
	mat4 model_matrix;
	mat4 view_matrix;
	mat4 projection_matrix;
	vec3 light_dir;
} ubo;



void main() {
	mat4 mvp = ubo.projection_matrix * ubo.view_matrix * ubo.model_matrix;
	gl_Position = mvp * vec4(pos, 1.0);
	vec3 normal = (ubo.model_matrix * vec4(norm, 1.0)).xyz;

	exposure = 1.0 + dot(normal, ubo.light_dir);
}