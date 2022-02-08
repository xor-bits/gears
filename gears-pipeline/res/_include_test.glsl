#version 420
#include<rand>
layout(location = 0) out vec4 col;

void main() {
    float x = rand(vec2(0.5, 0.5));
    col = vec4(x, x, x, 1.0);
}