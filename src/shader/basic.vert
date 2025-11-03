#version 460


layout(location = 0) in vec3 inPosition;
layout(location = 0) out vec3 outPosition;
layout(location = 1) in vec2 inTexCoords;
layout(location = 2) out vec2 outTexCoords;
layout(location = 3) out vec2 instance;


layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;


void main() {

    //vec3 scale = vec3(1920.0/1200.0, 1.0, 1.0);
    //gl_Position = ubo.proj * ubo.view * ubo.model * vec4(inPosition * scale, 1);

    int SIZE = 32;
    vec3 offset = vec3(gl_InstanceIndex%SIZE - float(SIZE)/2 + 0.5, gl_InstanceIndex/SIZE - float(SIZE)/2 + 0.5, 0.0);
    gl_Position = vec4(inPosition + offset * 2, SIZE);

    outPosition = gl_Position.xyz / gl_Position.w;
    outTexCoords = inTexCoords;
    instance = offset.xy/SIZE;

}