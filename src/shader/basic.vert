#version 460


layout(location = 0) in vec3 inPosition;
layout(location = 0) out vec3 outPosition;
layout(location = 1) in vec2 inTexCoords;
layout(location = 2) out vec2 outTexCoords;


layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;


void main() {

    vec3 scale = vec3(1920.0/1200.0, 1.0, 1.0);
    //gl_Position = ubo.proj * ubo.view * ubo.model * vec4(inPosition * scale, 1);
    gl_Position = vec4(inPosition, 1);
    outTexCoords = inTexCoords;
}