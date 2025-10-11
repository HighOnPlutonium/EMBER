#version 460


layout(location = 0) in vec3 inPosition;
layout(location = 0) out vec3 outPosition;

layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;


void main() {

    gl_Position = vec4(inPosition, 1.0);
    outPosition = gl_Position.xyz / gl_Position.w;
}