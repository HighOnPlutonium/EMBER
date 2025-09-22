#version 460
//hardcoded vertex data because i ain't dealing with vertex buffers for now
vec2 positions[4] = vec2[](
    vec2( 1.0, -1.0),
    vec2(-1.0, -1.0),
    vec2( 1.0,  1.0),
    vec2(-1.0,  1.0)
);

vec3 colors[4] = vec3[](
    vec3(1.0, 0.5, 0.0),
    vec3(0.25, 1.0, 0.25),
    vec3(0.0, 0.5, 1.0),
    vec3(0.75, 0.0, 0.75)
);

layout(location = 0) out vec3 vertex_color;

void main() {

    gl_Position  = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    vertex_color = colors[gl_VertexIndex];
}