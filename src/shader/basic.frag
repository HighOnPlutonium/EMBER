#version 460

layout(location = 0) out vec4 color;

layout(location = 0) in vec3 vertex_color;

void main() {
    ivec2 XY = ivec2(gl_FragCoord);
    int a = abs(XY.x - 250) & abs(XY.y-250);
    int b = ~XY.x & XY.y;
    int c = XY.x & ~XY.y;
    int predicate = a*b*c;
    if ( !(predicate == 0) ) discard;

    vec2 xy = gl_FragCoord.xy / vec2(500,500);
    //vec3 C = vec3(xy.x, 0.0, xy.y);
    vec3 C = vec3(1.0,1.0,1.0);
    float brightness = 0.2;
    color = vec4(C*brightness,brightness);
}