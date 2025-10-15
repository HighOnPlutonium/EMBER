#version 460

#include <lygia/generative/voronoi.glsl>

layout(location = 0) out vec4 color;
layout(location = 0) in vec3 outPosition;



layout(push_constant) uniform pc {
    vec2 rand;
    float aspect;
    float t;
    vec2 win_pos;
};

layout(binding = 1) uniform sampler2D tex;



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







void main() {
    float root_aspect = 1200.0/1920.0;
    float scale = 1.0;

    vec2 pos = outPosition.xy * vec2(aspect,1);
    vec3 C = vec3(0);
    float A = 1;



    /*
    const int SIZE = 5;
    float MULT = 1.0/256.0;
    float[SIZE][SIZE] kernel = {
        {1, 4,  6,  4,  1},
        {4, 16, 24, 16, 4},
        {6, 24, 36, 24, 6},
        {4, 16, 24, 16, 4},
        {1, 4,  6,  4,  1}};
    */

    const int SIZE = 3;
    float MULT = -1;
    float[SIZE][SIZE] kernel = {
        {  1,  1,  1 },
        {  1, -9,  1},
        {  1,  1,  1}};



    vec2 tex_pos = (outPosition.xy + vec2(1))/2 * vec2(root_aspect*aspect,1) * scale;

    for (int i = 0; i < SIZE; i++) {
    for (int j = 0; j < SIZE; j++) {
        C += MULT * kernel[i][j] * texelFetch(tex, ivec2(tex_pos*vec2(1920,1200)-roundEven(float(SIZE)/2.0))+ivec2(i,j), 0).xyz;
    }}

    color = vec4(C*A,A);

}