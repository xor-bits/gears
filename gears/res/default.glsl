#version 420
#extension GL_ARB_separate_shader_objects : enable

GEARS_IN(0, vec2 in_position)
GEARS_IN(1, vec3 in_color)

GEARS_VERT_UBO(0, #!ubo: UBO { time: f32 }#!)

GEARS_INOUT(0, vec3 color)

GEARS_OUT(0, vec4 out_color)



#if defined(GEARS_VERTEX)

void main() {
	gl_Position = vec4(in_position, 0.0, 1.0);
	color = in_color;
}

#elif defined(GEARS_FRAGMENT)

void main() {
	out_color = vec4(color, 1.0);
}

#endif