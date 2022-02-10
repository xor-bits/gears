#version 420

layout(location = 0) in float fi_exp;

layout(location = 0) out vec4 color;



void main() {
	vec3 c = vec3(1.0) * smoothstep(0.1, 1.9, fi_exp);
	color = vec4(c, 1.0);
}