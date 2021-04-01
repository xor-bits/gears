#version 420
#extension GL_ARB_separate_shader_objects : enable

layout(triangles) in;
layout(line_strip, max_vertices = 4) out;

#[gears_gen(in)]
struct VFSharedData {
	float exposure[];
} geom_in;

#[gears_gen(out)]
struct VFSharedData {
	float exposure;
} geom_out;



void vertex(int i)
{
	gl_Position = gl_in[i].gl_Position;
	geom_out.exposure = geom_in.exposure[i];
    EmitVertex();
}

void main() {
    vertex(0);
    vertex(1);
    vertex(2);
    vertex(0);
    EndPrimitive();
}