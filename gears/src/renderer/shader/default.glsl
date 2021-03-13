#version 410

float one() {
	return 1.0;
}

#if defined(SHADER_MODULE_VERTEX)

	// vertex shader
	layout(location = 0) in vec2 vert_position;
	layout(location = 1) in vec3 vert_color;

	layout(location = 0) out vec3 frag_color;



	void main() {
		gl_Position = vec4(vert_position, 0.0, one());
		frag_color = vert_color;
	}

#elif defined(SHADER_MODULE_FRAGMENT)

	// fragment shader
	layout(location = 0) in vec3 frag_color;

	layout(location = 0) out vec4 raster_color;



	void main() {
		raster_color = vec4(frag_color, one());
	}
#endif