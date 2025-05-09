#version 460

layout(location = 0) out vec4 color;

layout(location = 0) in vec3 vertex_color;

void main() {
    vec3 C = vertex_color;
    C = vec3(cross(C.zxy,C.yzx));
    float brightness = 0.1;
    color = vec4(C*brightness,brightness);
}