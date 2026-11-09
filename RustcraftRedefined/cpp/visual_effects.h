#ifndef RUSTCRAFT_VISUAL_EFFECTS_H
#define RUSTCRAFT_VISUAL_EFFECTS_H
#include <stdint.h>
#include <cmath>

// A shared analytic surface keeps adjacent chunk borders aligned. This is a
// visual displacement, not a fluid solver or a change to collision geometry.
inline float water_wave(double x,double z,double time){
    return (float)(-.09 + .045*std::sin(x*.22+z*.12+time*1.1) + .03*std::sin(z*.31-x*.08-time*.8));
}
struct Particle { float x=0,y=0,z=0,vx=0,vy=0,vz=0,life=0; uint8_t block=0; };
inline uint32_t particle_random(uint32_t& state){state^=state<<13;state^=state>>17;state^=state<<5;return state;}
inline void make_break_particle(Particle& p,int x,int y,int z,uint8_t block,uint32_t& state){
    auto next=[&](){return (particle_random(state)&65535)/65535.0f;};
    p.x=x+.2f+next()*.6f;p.y=y+.2f+next()*.6f;p.z=z+.2f+next()*.6f;
    p.vx=(next()-.5f)*2.5f;p.vy=1.4f+next()*2.0f;p.vz=(next()-.5f)*2.5f;
    p.life=.45f+next()*.45f;p.block=block;
}
inline void advance_particle(Particle& p,float dt){
    if(p.life<=0||dt<=0)return;if(dt>.05f)dt=.05f;
    p.life-=dt;p.vy-=9.f*dt;p.x+=p.vx*dt;p.y+=p.vy*dt;p.z+=p.vz*dt;
    if(p.life<0)p.life=0;
}
#endif
