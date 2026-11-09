#include "../cpp/visual_effects.h"
#include <cassert>
#include <cmath>
#include <cstdio>
int main(){
    for(int x=-64;x<=64;x++)for(int z=-64;z<=64;z++)for(int t=0;t<5;t++){
        float h=water_wave(x,z,t*.7);assert(h>=-.166f&&h<=-.014f);
        assert(h==water_wave((x-16)+16,z,t*.7));
    }
    assert(water_wave(10,20,0)!=water_wave(10,20,1));
    uint32_t a=12345,b=12345;Particle p,q;make_break_particle(p,-10,22,8,3,a);make_break_particle(q,-10,22,8,3,b);
    assert(p.x==q.x&&p.vy==q.vy&&p.block==3&&p.life>0&&p.life<=.9f);
    float y=p.y;advance_particle(p,.02f);assert(p.y>y&&p.life<q.life);
    for(int i=0;i<100;i++)advance_particle(p,.05f);assert(p.life==0);
    float x=p.x;advance_particle(p,.05f);assert(p.x==x);
    puts("PASS bounded animated waves, chunk-edge continuity, deterministic particles and lifetime");
}
