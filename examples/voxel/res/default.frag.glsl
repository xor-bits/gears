#version 420

layout(location = 0) in float fi_exp;

layout(location = 0) out vec4 color;



void main() {
	vec3 c = 
#if defined(DEBUGGING)
		vec3(fi_exp, fi_exp * 0.5, 0.0);
#else
		vec3(fi_exp);
#endif
	color = vec4(c, 1.0);
}