#version 420
layout(binding = 0) uniform UBO { vec3 col; } ubo;
#if defined(ENABLE)
void main() {}
#endif