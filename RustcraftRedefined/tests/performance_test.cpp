#include "../cpp/edit_store.h"
#include "../cpp/frustum.h"
#include <cassert>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <vector>
#include <unordered_map>
#include <random>
#include <chrono>
#include <climits>
struct Edit { int32_t x,y,z;uint8_t id; };
int main(){
    assert(chunk_coord(-1)==-1&&local_coord(-1)==15);assert(chunk_coord(-16)==-1&&local_coord(-16)==0);
    assert(chunk_coord(INT_MIN)==-134217728&&local_coord(INT_MIN)==0);assert(chunk_coord(INT_MAX)==134217727&&local_coord(INT_MAX)==15);
    EditStore store;store.set(0,-1,0,2);store.set(0,64,0,2);store.set(0,0,0,14);assert(store.size()==0);store.set(-1,2,-17,3);store.set(-1,2,-17,7);store.set(INT_MIN,63,INT_MAX,13);assert(store.size()==2);
    uint8_t blocks[16384]={};store.apply(-1,-2,blocks);assert(blocks[(2*16+15)*16+15]==7);
    bool found=false;store.each([&](int32_t x,int32_t y,int32_t z,uint8_t v){if(x==INT_MIN&&y==63&&z==INT_MAX&&v==13)found=true;});assert(found);
    EditStore second;second.swap(store);assert(store.size()==0&&second.size()==2);
    assert(chunk_in_frustum(0,0,8,33,8,0,0,1.5));assert(chunk_in_frustum(0,-4,8,33,8,0,0,1.5));assert(!chunk_in_frustum(0,8,8,33,8,0,0,1.5));
    // Any chunk with at least one corner inside the view must not be culled.
    std::mt19937 rng(42);std::uniform_real_distribution<double> angle(-179,179),position(-80,80),aspect(.5,3);
    for(int trial=0;trial<100000;trial++){
        int cx=(int)(rng()%25)-12,cz=(int)(rng()%25)-12;double px=position(rng),py=32+position(rng),pz=position(rng),yaw=angle(rng),pitch=angle(rng)*.45,asp=aspect(rng);
        double rad=.017453292519943295,sy=sin(yaw*rad),cy=cos(yaw*rad),sp=sin(pitch*rad),cp=cos(pitch*rad);bool corner_inside=false;
        for(int x=0;x<=16;x+=16)for(int y=0;y<=64;y+=64)for(int z=0;z<=16;z+=16){
            double dx=cx*16+x-px,dy=y-py,dz=cz*16+z-pz,right=dx*cy-dz*sy,up=dx*sy*sp+dy*cp+dz*cy*sp,forward=-dx*sy*cp+dy*sp-dz*cy*cp;
            if(forward>=.1&&forward<=180&&fabs(right)<=forward*.7*asp&&fabs(up)<=forward*.7)corner_inside=true;
        }
        if(corner_inside)assert(chunk_in_frustum(cx,cz,px,py,pz,yaw,pitch,asp));
    }
    puts("PASS indexed edits, negative/extreme coordinates, updates, and conservative frustum checks");
    EditStore indexed;std::vector<Edit> reference;reference.reserve(30000);
    for(int i=0;i<30000;i++){int x=(int)(rng()%65536)-32768,z=(int)(rng()%65536)-32768,y=(int)(rng()%64);uint8_t id=1+(rng()%13);indexed.set(x,y,z,id);reference.push_back({x,y,z,id});}
    std::vector<std::pair<int,int>> queries;for(int i=0;i<1000;i++){
        if(i%2==0){const auto& e=reference[rng()%reference.size()];queries.push_back({chunk_coord(e.x),chunk_coord(e.z)});}
        else queries.push_back({(int)(rng()%4096)-2048,(int)(rng()%4096)-2048});
    }
    EditStore restored;indexed.each([&](int32_t x,int32_t y,int32_t z,uint8_t id){restored.set(x,y,z,id);});assert(restored.size()==indexed.size());
    for(int i=0;i<50;i++){uint8_t a[16384]={},b[16384]={};auto query=queries[i];
        for(const auto&e:reference)if(chunk_coord(e.x)==query.first&&chunk_coord(e.z)==query.second)a[(e.y*16+local_coord(e.z))*16+local_coord(e.x)]=e.id;
        restored.apply(query.first,query.second,b);for(int j=0;j<16384;j++)assert(a[j]==b[j]);
    }
    using Clock=std::chrono::steady_clock;volatile unsigned sum=0;auto start=Clock::now();
    for(auto v:queries){uint8_t target[16384]={};for(const auto&e:reference)if(chunk_coord(e.x)==v.first&&chunk_coord(e.z)==v.second)target[(e.y*16+local_coord(e.z))*16+local_coord(e.x)]=e.id;for(auto b:target)sum+=b;}
    auto mid=Clock::now();for(auto v:queries){uint8_t target[16384]={};indexed.apply(v.first,v.second,target);for(auto b:target)sum+=b;}auto end=Clock::now();
    printf("1000 chunk edit lookups, 30000 scattered edits: scan %.2f ms; indexed apply %.2f ms (synthetic, not FPS)\n",std::chrono::duration<double,std::milli>(mid-start).count(),std::chrono::duration<double,std::milli>(end-mid).count());(void)sum;
}
