#version 420

layout(location = 0) in vec2 pos;

layout(binding = 0) uniform UBO {
    mat4 mvp;
} ubo;



void main() {
    gl_Position = ubo.mvp * vec4(pos, 0.0, 1.0);
}