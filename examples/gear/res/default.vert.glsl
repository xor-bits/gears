#version 420

layout(location = 0) in vec3 vi_pos;
layout(location = 1) in vec3 vi_norm;

layout(location = 0) out float fi_exp;

layout(binding = 0) uniform UBO {
	mat4 model_matrix;
	mat4 view_matrix;
	mat4 projection_matrix;
	vec3 light_dir;
} ubo;



void main() {
	mat4 mvp = ubo.projection_matrix * ubo.view_matrix * ubo.model_matrix;
	gl_Position = mvp * vec4(vi_pos, 1.0);
	vec3 normal = (ubo.model_matrix * vec4(vi_norm, 1.0)).xyz;

	fi_exp = 1.0 + dot(normal, ubo.light_dir);
}