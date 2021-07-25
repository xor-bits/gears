#version 420

layout(triangles) in;
layout(line_strip, max_vertices = 4) out;

layout(location = 0) in float in_exposure[];
layout(location = 0) out float out_exposure;



void vertex(int i)
{
	gl_Position = gl_in[i].gl_Position;
	out_exposure = in_exposure[i];
    EmitVertex();
}

void main() {
    vertex(0);
    vertex(1);
    vertex(2);
    vertex(0);
    EndPrimitive();
}