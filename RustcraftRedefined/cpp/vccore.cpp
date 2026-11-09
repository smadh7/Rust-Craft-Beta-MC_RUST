#include <stdint.h>
#include <stddef.h>

// All arithmetic in the hash is deliberately unsigned and wraps deterministically.
static uint32_t hash2(int32_t x, int32_t z, uint64_t seed) {
    uint64_t n = (uint64_t)(uint32_t)x * 0x9e3779b185ebca87ULL ^ (uint64_t)(uint32_t)z * 0xc2b2ae3d27d4eb4fULL ^ seed;
    n ^= n >> 30; n *= 0xbf58476d1ce4e5b9ULL; n ^= n >> 27;
    n *= 0x94d049bb133111ebULL; n ^= n >> 31;
    return (uint32_t)n;
}
static int32_t floor_div(int32_t x, int32_t n) { int32_t q=x/n, r=x%n; return q-(r<0); }
static float noise(int32_t x, int32_t z, int32_t scale, uint64_t seed) {
    int32_t gx=floor_div(x,scale), gz=floor_div(z,scale);
    float fx=(float)(x-gx*scale)/scale, fz=(float)(z-gz*scale)/scale;
    fx=fx*fx*(3-2*fx); fz=fz*fz*(3-2*fz);
    float a=(hash2(gx,gz,seed)&65535)/65535.0f, b=(hash2(gx+1,gz,seed)&65535)/65535.0f;
    float c=(hash2(gx,gz+1,seed)&65535)/65535.0f, d=(hash2(gx+1,gz+1,seed)&65535)/65535.0f;
    return (a+(b-a)*fx)*(1-fz)+(c+(d-c)*fx)*fz;
}
extern "C" int32_t vc_terrain_height(int32_t x,int32_t z,uint64_t seed) {
    return 17+(int32_t)(noise(x,z,96,seed)*20+noise(x,z,24,seed+1)*9);
}
extern "C" void vc_generate_chunk(int32_t cx,int32_t cz,uint64_t seed,uint8_t *out) {
    for(int z=0;z<16;z++) for(int x=0;x<16;x++) {
        int32_t wx=cx*16+x,wz=cz*16+z, h=vc_terrain_height(wx,wz,seed);
        for(int y=0;y<64;y++) {
        uint8_t b=0;
        if(y==0) b=9;
        else if(y<h-3) b=1;
        else if(y<h) b=2;
        else if(y==h) b=h<=27?4:(h>=42?11:3);
        else if(y<=26) b=5;
        out[(y*16+z)*16+x]=b;
        }
    }
}
// Padded input: x,z in 0..17, y in 0..65; interior is offset by one.
// Each quad uses two words: x(5), y(7), z(5), face(3), block(8);
// second word holds width(7), height(7). Face order: -X,+X,-Y,+Y,-Z,+Z.
// Return required word count even when capacity is too small; never write past capacity.
extern "C" uint32_t vc_mesh_chunk(const uint8_t* p,uint32_t* out,uint32_t capacity) {
    uint32_t count=0; uint8_t mask[64*16];
    auto at=[&](int x,int y,int z)->uint8_t { return p[((y+1)*18+(z+1))*18+(x+1)]; };
    for(int face=0;face<6;face++) {
        int axis=face/2, sign=(face&1)?1:-1;
        int depth=axis==1?64:16, w=16, h=axis==1?16:64;
        for(int s=0;s<depth;s++) {
            for(int v=0;v<h;v++) for(int u=0;u<w;u++) {
                int x=axis==0?s:u, y=axis==1?s:v, z=axis==2?s:(axis==1?v:u);
                uint8_t b=at(x,y,z), n=at(x+(axis==0?sign:0),y+(axis==1?sign:0),z+(axis==2?sign:0));
                bool trans=b==5||b==10, nt=n==5||n==10;
                mask[v*w+u]=(b && (!n || (nt && n!=b && !trans)))?b:0;
            }
            for(int v=0;v<h;v++) for(int u=0;u<w;) {
                uint8_t b=mask[v*w+u]; if(!b){u++;continue;}
                int qw=1,qh=1; while(u+qw<w&&mask[v*w+u+qw]==b) qw++;
                bool ok=true; while(v+qh<h&&ok) { for(int i=0;i<qw;i++) if(mask[(v+qh)*w+u+i]!=b){ok=false;break;} if(ok) qh++; }
                for(int j=0;j<qh;j++) for(int i=0;i<qw;i++) mask[(v+j)*w+u+i]=0;
                int x=axis==0?s:u, y=axis==1?s:v, z=axis==2?s:(axis==1?v:u);
                uint32_t a=(uint32_t)x | ((uint32_t)y<<5) | ((uint32_t)z<<12) | ((uint32_t)face<<17) | ((uint32_t)b<<20);
                uint32_t bword=(uint32_t)qw|((uint32_t)qh<<7);
                if(count+1<capacity && out){out[count]=a;out[count+1]=bword;} count+=2; u+=qw;
            }
        }
    }
    return count;
}
