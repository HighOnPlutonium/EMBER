#version 460

#include <lygia/generative/voronoi.glsl>

layout(location = 0) out vec4 color;
layout(location = 0) in vec3 outPosition;

layout(push_constant) uniform pc {
    vec2 rand;
    float aspect;
    float t;
};



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
    vec2 pos = outPosition.xy * vec2(aspect,1);
    vec3 C = vec3(0);
    float A = 1;


    float r1 = hashRandom(pos*t);
    float r2 = hashRandom(pos*(floor(t*20)/20));
    //float manhattan = abs(pos.x) + abs(pos.y);
    float chebyshev = max(abs(pos.x),abs(pos.y));
    C = vec3(r2);
    if (chebyshev < 0.5) C = vec3(r1);
    A = 1;

    color = vec4(C*A,A);
}