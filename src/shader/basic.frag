#version 460
#extension GL_EXT_shader_explicit_arithmetic_types_int64 : enable

#include <lygia/generative/voronoi.glsl>
#include <lygia/color/hueShift.glsl>


layout(location = 0) out vec4 color;
layout(location = 0) in vec3 outPosition;
layout(location = 2) in vec2 outTexCoords;


layout(push_constant) uniform pc {
    vec2 rand;
    float aspect;
    float t;
    int id;
};

layout(binding = 1) uniform sampler2D tex;

layout(std140, binding = 2) buffer MVBufferObject {
    uint64_t size;
    ivec2 dims;
    uint[] data;
} mv;



// A single iteration of Bob Jenkins' One-At-A-Time hashing algorithm.
uint hash( uint x ) {
    x += ( x << 10u );
    x ^= ( x >>  6u );
    x += ( x <<  3u );
    x ^= ( x >> 11u );
    x += ( x << 15u );
    return x;
}



// Compound versions of the hashing algorithm I whipped together.
uint hash( uvec2 v ) { return hash( v.x ^ hash(v.y)                         ); }
uint hash( uvec3 v ) { return hash( v.x ^ hash(v.y) ^ hash(v.z)             ); }
uint hash( uvec4 v ) { return hash( v.x ^ hash(v.y) ^ hash(v.z) ^ hash(v.w) ); }



// Construct a float with half-open range [0:1] using low 23 bits.
// All zeroes yields 0.0, all ones yields the next smallest representable value below 1.0.
float floatConstruct( uint m ) {
    const uint ieeeMantissa = 0x007FFFFFu; // binary32 mantissa bitmask
    const uint ieeeOne      = 0x3F800000u; // 1.0 in IEEE binary32

    m &= ieeeMantissa;                     // Keep only mantissa bits (fractional part)
    m |= ieeeOne;                          // Add fractional part to 1.0

    float  f = uintBitsToFloat( m );       // Range [1:2]
    return f - 1.0;                        // Range [0:1]
}



// Pseudo-random value in half-open range [0:1].
float hashRandom( float x ) { return floatConstruct(hash(floatBitsToUint(x))); }
float hashRandom( vec2  v ) { return floatConstruct(hash(floatBitsToUint(v))); }
float hashRandom( vec3  v ) { return floatConstruct(hash(floatBitsToUint(v))); }
float hashRandom( vec4  v ) { return floatConstruct(hash(floatBitsToUint(v))); }




float[3][3] mult(float S, float[3][3] arr) {
    float[3][3] ret = arr;
    for (int i = 0; i < 3; i++) {
    for (int j = 0; j < 3; j++) {
        ret[j][i] *= S;
    }}
    return ret;
}
vec3[3][3] mult(float S, vec3[3][3] arr) {
    vec3[3][3] ret = arr;
    for (int i = 0; i < 3; i++) {
        for (int j = 0; j < 3; j++) {
            ret[j][i] *= S;
        }}
    return ret;
}
float[5][5] mult(float S, float[5][5] arr) {
    float[5][5] ret = arr;
    for (int i = 0; i < 5; i++) {
        for (int j = 0; j < 5; j++) {
            ret[j][i] *= S;
        }}
    return ret;
}
vec3[5][5] mult(float S, vec3[5][5] arr) {
    vec3[5][5] ret = arr;
    for (int i = 0; i < 5; i++) {
        for (int j = 0; j < 5; j++) {
            ret[j][i] *= S;
        }}
    return ret;
}


vec3 convolve3(vec3[5][5] kernel, sampler2D tex, vec2 pos) {
    vec3 ret = vec3(0);
    for (int i = 0; i < 5; i++) {
    for (int j = 0; j < 5; j++) {
        ret += kernel[j][i] * texelFetch(tex, ivec2(pos*vec2(1920,1200)-2)+ivec2(i,j), 0).xyz;
    }}
    return ret;
}
vec3 convolve3(float[5][5] kernel, sampler2D tex, vec2 pos) {
    vec3 ret = vec3(0);
    for (int i = 0; i < 5; i++) {
        for (int j = 0; j < 5; j++) {
            ret += kernel[j][i] * texelFetch(tex, ivec2(pos*vec2(1920,1200)-2)+ivec2(i,j), 0).xyz;
        }}
    return ret;
}
float convolve(float[5][5] kernel, sampler2D tex, vec2 pos) {
    float ret = 0;
    for (int i = 0; i < 5; i++) {
        for (int j = 0; j < 5; j++) {
            ret += kernel[j][i] * dot(texelFetch(tex, ivec2(pos*vec2(1920,1200)-2)+ivec2(i,j), 0).xyz, vec3(1.0/3.0));
        }}
    return ret;
}
vec3 convolve3(vec3[3][3] kernel, sampler2D tex, vec2 pos) {
    vec3 ret = vec3(0);
    for (int i = 0; i < 3; i++) {
        for (int j = 0; j < 3; j++) {
            ret += kernel[j][i] * texelFetch(tex, ivec2(pos*vec2(1920,1200)-1)+ivec2(i,j), 0).xyz;
        }}
    return ret;
}
vec3 convolve3(float[3][3] kernel, sampler2D tex, vec2 pos) {
    vec3 ret = vec3(0);
    for (int i = 0; i < 3; i++) {
        for (int j = 0; j < 3; j++) {
            ret += kernel[j][i] * texelFetch(tex, ivec2(pos*vec2(1920,1200)-1)+ivec2(i,j), 0).xyz;
        }}
    return ret;
}
float convolve(float[3][3] kernel, sampler2D tex, vec2 pos) {
    float ret = 0;
    for (int i = 0; i < 3; i++) {
        for (int j = 0; j < 3; j++) {
            ret += kernel[j][i] * dot(texelFetch(tex, ivec2(pos*vec2(1920,1200)-1)+ivec2(i,j), 0).xyz, vec3(1.0/3.0));
        }}
    return ret;
}


float convolve(float[11][11] kernel, sampler2D tex, vec2 pos) {
    float ret = 0;
    for (int i = 0; i < 11; i++) {
        for (int j = 0; j < 11; j++) {
            ret += kernel[j][i] * dot(texelFetch(tex, ivec2(pos*vec2(1920,1200)-5)+ivec2(i,j), 0).xyz, vec3(1.0/3.0));
        }}
    return ret;
}



float gs(vec3 c) {
    return dot(c,vec3(1.0/3.0));
}

float chebyshev(vec2 a, vec2 b) { return max(abs(b.x - a.x), abs(b.y - a.y)); }
float chebyshev(vec3 a, vec3 b) { return max(chebyshev(a.xy, b.xy), abs(b.z - a.z)); }
float taxicab(vec2 a, vec2 b) { return abs(b.x - a.x) + abs(b.y - a.y); }
float taxicab(vec3 a, vec3 b) { return taxicab(a.xy, b.xy) + abs(b.z - a.z); }


void main() {
    vec2 pos = outPosition.xy * vec2(aspect,1);
    vec3 C = vec3(0);
    float A = 1;

    /*
    vec2 tex_pos = ((outPosition.xy + vec2(1))/2  * 1151.0/1200.0) * vec2((1200.0/1920.0)*aspect,1);

    vec3[3][3] identity = {
        {vec3( 0),vec3( 0),vec3( 0)},
        {vec3( 0),vec3( 1),vec3( 0)},
        {vec3( 0),vec3( 0),vec3( 0)}};

    float[3][3] box = {
        {1,1,1},
        {1,1,1},
        {1,1,1}};


    float[5][5] gauss = {
        {1, 4,  6,  4,  1},
        {4, 16, 24, 16, 4},
        {6, 24, 36, 24, 6},
        {4, 16, 24, 16, 4},
        {1, 4,  6,  4,  1}};
    gauss = mult(0.00390625, gauss);


    vec3[3][3] emboss = {
        {vec3( 2,0,0),vec3( 1,0,0),vec3( 0,0,0)},
        {vec3( 0,0,0),vec3( 1,1,1),vec3( 0,0,0)},
        {vec3( 0,0,0),vec3(-1,0,0),vec3(-2,0,0)}};
    emboss = mult(0.75, emboss);

    float[3][3] sobel1 = {
        {-3,0,3},
        {-10,0,10},
        {-3,0,3}};
    float[3][3] sobel2 = {
        {-3,-10,-3},
        {0,0,0},
        {3,10,3}};

    float[3][3] ridge = {
        {0,1,0},
        {1,-4,1},
        {0,1,0}};
    float[3][3] edge = {
        {1,1,1},
        {1,-8,1},
        {1,1,1}};
    */


    ivec2 dim = mv.dims;
    int X = int(gl_FragCoord.x);
    int Y = int(gl_FragCoord.y);
    int x = 0;
    int y = 0;
    int margin = 16;
    int scale = 3;

    //1920x1122

    uint c = 0;
    C = vec3(0.05, 0.0, 0.0875);
    if (margin <= Y && Y < dim.y*scale) {
        C = vec3(0.0475, 0.0, 0.125);
        if (margin <= X && X < (dim.x*scale + margin)) {
            x = (X - margin)/scale;
            y = (Y - margin)/scale;
            c = mv.data[x + y*dim.y];
            c = mv.data[0];
            C = vec3(c);
        } else if ((dim.x*scale + 2*margin) <= X && X < (2*dim.x*scale + 2*margin)) {
            x = (X - dim.x*scale - 2*margin)/scale;
            y = (Y - margin)/scale;
            c = mv.data[x + y*dim.y];
            c = mv.data[1];
            C = vec3(c);
        }
    }

    color = vec4(C*A,A);
}

