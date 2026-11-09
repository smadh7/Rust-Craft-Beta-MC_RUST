#include <stdint.h>
#include <cassert>
#include <cstdio>
#include <vector>
extern "C" void vc_generate_chunk(int32_t,int32_t,uint64_t,uint8_t*);
extern "C" uint32_t vc_mesh_chunk(const uint8_t*,uint32_t*,uint32_t);
static int pi(int x,int y,int z){return (y*18+z)*18+x;}
int main(){
    std::vector<uint8_t> pad(18*18*66);pad[pi(1,2,1)]=5;
    uint32_t n=vc_mesh_chunk(pad.data(),nullptr,0);std::vector<uint32_t> q(n);vc_mesh_chunk(pad.data(),q.data(),n);
    int top=0;for(unsigned i=0;i<n;i+=2)if(((q[i]>>20)&255)==5&&((q[i]>>17)&7)==3)top++;assert(top==1);
    pad[pi(1,3,1)]=5;n=vc_mesh_chunk(pad.data(),nullptr,0);q.resize(n);vc_mesh_chunk(pad.data(),q.data(),n);
    top=0;for(unsigned i=0;i<n;i+=2)if(((q[i]>>20)&255)==5&&((q[i]>>17)&7)==3){top++;assert(((q[i]>>5)&127)==2);}assert(top==1);
    std::vector<uint8_t> block(16384);
    for(int cz=-4;cz<=4;cz++)for(int cx=-4;cx<=4;cx++){
        vc_generate_chunk(cx,cz,12345,block.data());std::fill(pad.begin(),pad.end(),0);
        for(int y=0;y<64;y++)for(int z=0;z<16;z++)for(int x=0;x<16;x++)pad[pi(x+1,y+1,z+1)]=block[(y*16+z)*16+x];
        n=vc_mesh_chunk(pad.data(),nullptr,0);q.resize(n);assert(vc_mesh_chunk(pad.data(),q.data(),n)==n);
        for(unsigned i=0;i<n;i+=2)if(((q[i]>>20)&255)==5&&((q[i]>>17)&7)==3){
            unsigned x=q[i]&31,z=(q[i]>>12)&31,w=q[i+1]&127,h=(q[i+1]>>7)&127;
            assert(w&&h&&x+w<=16&&z+h<=16);
        }
    }
    puts("PASS water top culling and tessellation grid bounds");
}
