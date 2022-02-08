#version 420

layout(location = 0) in float in_exposure;

layout(location = 0) out vec4 out_col;



void main() {
    vec3 c = 
#if defined(DEBUGGING)
        vec3(in_exposure, in_exposure * 0.5, 0.0);
#else
        vec3(in_exposure);
#endif
    out_col = vec4(c, 1.0);
}