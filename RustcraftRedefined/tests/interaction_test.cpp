#include "../cpp/interaction.h"
#include <cassert>
#include <cmath>
#include <cstdio>
static uint8_t map(int x,int y,int z){return (x==0&&y==2&&z==-2)?1:0;}
int main(){
    ViewDirection up=view_direction(0,45),down=view_direction(0,-45);
    assert(up.y>0&&down.y<0&&up.z<0);
    RayHit h=trace_ray(.5,2.5,.5,view_direction(0,0),6,map);
    assert(h.yes&&h.x==0&&h.y==2&&h.z==-2&&h.has_previous&&h.prevz==-1);
    RayHit inside=trace_ray(.5,2.5,-1.5,view_direction(0,0),6,map);
    assert(inside.yes&&!inside.has_previous);
    RayHit miss=trace_ray(.5,2.5,.5,view_direction(0,0),1,map);assert(!miss.yes);
    assert(!placement_clear(.5,2,.5,0,2,0));assert(placement_clear(.5,2,.5,0,4,0));
    auto neg=[](int x,int y,int z)->uint8_t{return x==-2&&y==0&&z==0?1:0;};
    RayHit n=trace_ray(-.5,.5,.5,view_direction(90,0),6,neg);assert(n.yes&&n.x==-2&&n.prevx==-1);
    puts("PASS camera direction, raycast, reach, negative coordinates, placement safety");
}
