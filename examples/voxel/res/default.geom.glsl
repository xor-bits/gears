#version 420

layout(triangles) in;
layout(line_strip, max_vertices = 4) out;

layout(location = 0) in float gi_exp[];
layout(location = 0) out float fi_exp;



void vertex(int i)
{
	gl_Position = gl_in[i].gl_Position;
	fi_exp = gi_exp[i];
    EmitVertex();
}

void main() {
    vertex(0);
    vertex(1);
    vertex(2);
    vertex(0);
    EndPrimitive();
}