#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <vector>
#include <chrono>
extern "C" void vc_generate_chunk(int32_t,int32_t,uint64_t,uint8_t*);
extern "C" uint32_t vc_mesh_chunk(const uint8_t*,uint32_t*,uint32_t);
int main(){
    uint8_t a[16384],b[16384];vc_generate_chunk(-1,2,42,a);vc_generate_chunk(-1,2,42,b);
    if(memcmp(a,b,sizeof a)){puts("FAIL deterministic generation");return 1;}
    std::vector<uint8_t> p(18*18*66,0);
    for(int y=1;y<=64;y++)for(int z=1;z<=16;z++)for(int x=1;x<=16;x++)p[(y*18+z)*18+x]=1;
    if(vc_mesh_chunk(p.data(),nullptr,0)!=12){puts("FAIL cube mesh");return 1;}
    for(int y=1;y<=64;y++)for(int z=1;z<=16;z++)p[(y*18+z)*18]=1;
    if(vc_mesh_chunk(p.data(),nullptr,0)!=10){puts("FAIL neighbour culling");return 1;}
    uint32_t guard[3]={0xdeadbeef,0xdeadbeef,0xdeadbeef};if(vc_mesh_chunk(p.data(),guard+1,1)!=10||guard[0]!=0xdeadbeef||guard[1]!=0xdeadbeef||guard[2]!=0xdeadbeef){puts("FAIL capacity guard");return 1;}
    puts("PASS deterministic generation, greedy cube, neighbour culling, output bounds");
    using Clock=std::chrono::steady_clock;auto start=Clock::now();std::vector<uint8_t> chunks(256*16384);for(int i=0;i<256;i++)vc_generate_chunk(i%16,i/16,12345,&chunks[i*16384]);auto mid=Clock::now();uint64_t words=0;
    for(int i=0;i<256;i++){memset(p.data(),0,p.size());for(int y=0;y<64;y++)for(int z=0;z<16;z++)memcpy(&p[((y+1)*18+z+1)*18+1],&chunks[i*16384+(y*16+z)*16],16);words+=vc_mesh_chunk(p.data(),nullptr,0);}
    auto end=Clock::now();printf("256 chunks: generation %.2f ms; mesh sizing %.2f ms; %llu quads\n",std::chrono::duration<double,std::milli>(mid-start).count(),std::chrono::duration<double,std::milli>(end-mid).count(),(unsigned long long)(words/2));
}
