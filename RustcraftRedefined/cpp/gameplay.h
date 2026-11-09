#ifndef RUSTCRAFT_GAMEPLAY_H
#define RUSTCRAFT_GAMEPLAY_H
#include <stdint.h>
#include <algorithm>

enum GameMode : uint8_t { CREATIVE=0, SURVIVAL=1, HARDCORE=2 };
struct PlayerState {
    GameMode mode=CREATIVE;
    uint8_t health=20, hunger=20, selected=0;
    bool dead=false, has_position=false;
    uint16_t blocks[14]={};
    uint8_t hotbar[9]={1,2,3,4,6,8,10,12,13};
    uint16_t berries=0;
    double x=8,y=48,z=8;
};
inline bool limited(const PlayerState& p){return p.mode!=CREATIVE;}
inline bool harvest(PlayerState& p,uint8_t block){
    if(!block||block==5||block==9||block>13)return false;
    if(!limited(p))return true;
    if(p.blocks[block]>=999)return false;
    ++p.blocks[block];return true;
}
inline bool consume_block(PlayerState& p,uint8_t block){
    if(!block||block>13)return false;
    if(!limited(p))return true;
    if(!p.blocks[block])return false;
    --p.blocks[block];return true;
}
inline bool craft_planks(PlayerState& p){
    if(!limited(p)||!p.blocks[6]||p.blocks[8]>995)return false;
    --p.blocks[6];p.blocks[8]+=4;return true;
}
inline bool eat_berries(PlayerState& p){
    if(!limited(p)||!p.berries||p.hunger>=20)return false;
    --p.berries;p.hunger=(uint8_t)std::min(20,(int)p.hunger+4);return true;
}
inline bool damage(PlayerState& p,int points){
    if(!limited(p)||p.dead||points<=0)return false;
    p.health=(uint8_t)std::max(0,(int)p.health-points);
    if(!p.health&&p.mode==HARDCORE)p.dead=true;
    return p.health==0;
}
inline void respawn(PlayerState& p){
    if(p.mode!=SURVIVAL)return;
    p.health=20;p.hunger=20;p.dead=false;p.berries=0;
    for(auto& n:p.blocks)n=0;
}
#endif
