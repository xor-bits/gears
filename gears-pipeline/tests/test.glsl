#version 460
#if VALUE == 2

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