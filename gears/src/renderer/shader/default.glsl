#version 410

GEARS_IN(0, vec2 vert_position)
GEARS_IN(1, vec3 vert_color)

GEARS_INOUT(0, vec3 frag_color)

GEARS_OUT(0, vec4 raster_color)



#if defined(GEARS_VERTEX)

void main() {
	gl_Position = vec4(vert_position, 0.0, 1.0);
	frag_color = vert_color;
}

#elif defined(GEARS_FRAGMENT)

void main() {
	raster_color = vec4(frag_color, 1.0);
}

#endif