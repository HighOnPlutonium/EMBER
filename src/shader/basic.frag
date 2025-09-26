#version 460



//////////////// K.jpg's Re-oriented 4-Point BCC Noise (OpenSimplex2) ////////////////
////////////////////// Output: vec4(dF/dx, dF/dy, dF/dz, value) //////////////////////

// Inspired by Stefan Gustavson's noise
vec4 permute(vec4 t) {
    return t * (t * 34.0 + 133.0);
}

// Gradient set is a normalized expanded rhombic dodecahedron
vec3 grad(float hash) {

    // Random vertex of a cube, +/- 1 each
    vec3 cube = mod(floor(hash / vec3(1.0, 2.0, 4.0)), 2.0) * 2.0 - 1.0;

    // Random edge of the three edges connected to that vertex
    // Also a cuboctahedral vertex
    // And corresponds to the face of its dual, the rhombic dodecahedron
    vec3 cuboct = cube;
    cuboct[int(hash / 16.0)] = 0.0;

    // In a funky way, pick one of the four points on the rhombic face
    float type = mod(floor(hash / 8.0), 2.0);
    vec3 rhomb = (1.0 - type) * cube + type * (cuboct + cross(cube, cuboct));

    // Expand it so that the new edges are the same length
    // as the existing ones
    vec3 grad = cuboct * 1.22474487139 + rhomb;

    // To make all gradients the same length, we only need to shorten the
    // second type of vector. We also put in the whole noise scale constant.
    // The compiler should reduce it into the existing floats. I think.
    grad *= (1.0 - 0.042942436724648037 * type) * 32.80201376986577;

    return grad;
}

// BCC lattice split up into 2 cube lattices
vec4 openSimplex2Base(vec3 X) {

    // First half-lattice, closest edge
    vec3 v1 = round(X);
    vec3 d1 = X - v1;
    vec3 score1 = abs(d1);
    vec3 dir1 = step(max(score1.yzx, score1.zxy), score1);
    vec3 v2 = v1 + dir1 * sign(d1);
    vec3 d2 = X - v2;

    // Second half-lattice, closest edge
    vec3 X2 = X + 144.5;
    vec3 v3 = round(X2);
    vec3 d3 = X2 - v3;
    vec3 score2 = abs(d3);
    vec3 dir2 = step(max(score2.yzx, score2.zxy), score2);
    vec3 v4 = v3 + dir2 * sign(d3);
    vec3 d4 = X2 - v4;

    // Gradient hashes for the four points, two from each half-lattice
    vec4 hashes = permute(mod(vec4(v1.x, v2.x, v3.x, v4.x), 289.0));
    hashes = permute(mod(hashes + vec4(v1.y, v2.y, v3.y, v4.y), 289.0));
    hashes = mod(permute(mod(hashes + vec4(v1.z, v2.z, v3.z, v4.z), 289.0)), 48.0);

    // Gradient extrapolations & kernel function
    vec4 a = max(0.5 - vec4(dot(d1, d1), dot(d2, d2), dot(d3, d3), dot(d4, d4)), 0.0);
    vec4 aa = a * a; vec4 aaaa = aa * aa;
    vec3 g1 = grad(hashes.x); vec3 g2 = grad(hashes.y);
    vec3 g3 = grad(hashes.z); vec3 g4 = grad(hashes.w);
    vec4 extrapolations = vec4(dot(d1, g1), dot(d2, g2), dot(d3, g3), dot(d4, g4));

    // Derivatives of the noise
    vec3 derivative = vec3(-8.0 * mat4x3(d1, d2, d3, d4) * (aa * a * extrapolations)
    + mat4x3(g1, g2, g3, g4) * aaaa);

    // Return it all as a vec4
    return vec4(derivative, dot(aaaa, extrapolations));
}

// Use this if you don't want Z to look different from X and Y
vec4 openSimplex2_Conventional(vec3 X) {

    // Rotate around the main diagonal. Not a skew transform.
    vec4 result = openSimplex2Base(dot(X, vec3(2.0/3.0)) - X);
    return vec4(dot(result.xyz, vec3(2.0/3.0)) - result.xyz, result.w);
}

// Use this if you want to show X and Y in a plane, then use Z for time, vertical, etc.
vec4 openSimplex2_ImproveXY(vec3 X) {

    // Rotate so Z points down the main diagonal. Not a skew transform.
    mat3 orthonormalMap = mat3(
    0.788675134594813, -0.211324865405187, -0.577350269189626,
    -0.211324865405187, 0.788675134594813, -0.577350269189626,
    0.577350269189626, 0.577350269189626, 0.577350269189626);

    vec4 result = openSimplex2Base(orthonormalMap * X);
    return vec4(result.xyz * orthonormalMap, result.w);
}

//////////////////////////////// End noise code ////////////////////////////////







//const float PI = 3.14159265358979323846264338327950288419716939937510582097494459230781640628620899862803482534211706798214808651328230664709384460955058223172535940812848111745028410270193852110555964462294895493038196442881097566593344612847564823378678316527120190914564856692346034861045432664821339360726024914127372458700660631558817488152092096282925409171536436789259036001133053054882046652138414695194151160943305727036575959195309218611738193261179310511854807446237996274956735188575272489122793818301194912983367336244065664308602139494639522473719070217986094370277053921717629317675238467481846766940513200056812714526356082778577134275778960917363717872146844090122495343014654958537105079227968925892354201995611212902196086403441815981362977477130996051870721134999999837297804995105973173281609631859502445945534690830264252230825334468503526193118817101000313783875288658753320838142061717766914730359825349042875546873115956286388235378759375195778185778053217122680661300192787661119590921642019893809525720106548586327;
const float PI = 3.1415926535;

layout(push_constant) uniform pc {
    vec2 rand;
    float aspect;
    float t;
};

layout(location = 0) out vec4 color;
layout(location = 0) in vec3 outPosition;

float random(vec2 pos) {
    return float( int(fract(sin(pos.x)*32768)*100000000) & int(fract(sin(pos.y)*32768)*100000000) )/100000000;
}

vec2 _pos(vec2 pos, float res) { return round(pos*res)/res; }

void main() {
    vec2 pos = outPosition.xy * vec2(aspect,1);
    vec3 C = vec3(0);
    float A = 1;
    /*
    float circle = abs(sin(10*(pow(pos.x,2)+pow(pos.y,2))-2*PI*t));
    C = vec3(smoothstep(0.99,1,circle));

    float d = length(pos);
    float E = 0.001;
    vec2 R1 = vec2(0.0, 0.1);
    vec2 R2 = vec2(0.9, 1.0);

    float limit = abs(2*log(E/(1-E)));

    float exponent1 =  ( d - dot(R1,vec2(0.5)) ) / dot(R1,vec2(-1,1));
    float exponent2 =  ( dot(R2,vec2(0.5)) - d ) / dot(R2,vec2(-1,1));

    float brightness = clamp(dot(1/(1+exp(-limit*vec2(exponent1,exponent2))),vec2(1))-1,0,1);
    float mask = C.x*brightness;

    color = vec4(vec3((1-mask)*random(pos) + mask*random(pos)),1);
*/
    float delta = 0.2;
    float margin = 0;
    vec4 s0 = openSimplex2_ImproveXY(vec3(pos,0));
    vec4 s1 = openSimplex2_ImproveXY(vec3(pos,delta));
    vec4 s2 = openSimplex2_ImproveXY(vec3(pos,delta*2));
    float v0 = int(length(max(s0.xyz,0)) > margin);
    float v1 = int(length(max(s1.xyz,0)) > margin);
    float v2 = int(length(max(s2.xyz,0)) > margin);
    float m0 = 1 - v0;
    float m1 = 1 - v1;
    float m2 = 1 - v2;


    vec4 simplex = s0 + s1*m0;
    vec3 normalDirection = normalize(simplex.xyz);
    vec3 lightDirection = normalize(vec3(0,0,1));

    C = vec3(v0, v1*(1 - smoothstep(0,2,length(max(s0.xyz,0)))), 1);
    vec3 diffuseReflection = C * max(0.0, dot(normalDirection, lightDirection));
    C = diffuseReflection;

    color = vec4(C,A);
}