#ifndef RUSTCRAFT_WORLD_FORMAT_H
#define RUSTCRAFT_WORLD_FORMAT_H
#include "gameplay.h"
#include <stdint.h>
#include <string.h>

// Version 1 stores edits only; versions 2 and 3 include a 64-byte player
// record. Version 3 also stores hotbar assignments. All have 13-byte edits.
// Supported Windows builds are little-endian.
inline void encode_world_header(unsigned char (&header)[20],uint64_t seed,uint32_t count,bool v2=false){
    memcpy(header,v2?"RCRSAVE3":"RCRSAVE1",8);memcpy(header+8,&seed,8);memcpy(header+16,&count,4);
}
inline bool decode_world_header(const unsigned char (&header)[20],int64_t length,uint64_t& seed,uint32_t& count,bool* v2=nullptr){
    bool version2=memcmp(header,"RCRSAVE2",8)==0||memcmp(header,"RCRSAVE3",8)==0;
    if(!version2&&memcmp(header,"RCRSAVE1",8)!=0)return false;
    memcpy(&seed,header+8,8);memcpy(&count,header+16,4);
    if(v2)*v2=version2;
    return count<=1000000&&length==20LL+(version2?64LL:0LL)+13LL*count;
}
inline void encode_player(unsigned char (&out)[64],const PlayerState& p){
    memset(out,0,64);out[0]=(uint8_t)p.mode;out[1]=p.dead;out[2]=p.health;out[3]=p.hunger;out[4]=(uint8_t)((p.selected&15)|(p.hotbar[8]<<4));out[5]=p.has_position;
    for(int i=0;i<14;i++)memcpy(out+6+i*2,&p.blocks[i],2);
    memcpy(out+34,&p.berries,2);memcpy(out+36,&p.x,8);memcpy(out+44,&p.y,8);memcpy(out+52,&p.z,8);
    for(int i=0;i<8;i+=2)out[60+i/2]=(uint8_t)(p.hotbar[i]|(p.hotbar[i+1]<<4));
}
inline bool decode_player(const unsigned char (&in)[64],PlayerState& p){
    if(in[0]>HARDCORE||in[1]>1||in[2]>20||in[3]>20||(in[4]&15)>8||in[5]>1)return false;
    PlayerState q;q.mode=(GameMode)in[0];q.dead=in[1]!=0;q.health=in[2];q.hunger=in[3];q.selected=in[4]&15;q.has_position=in[5]!=0;
    if((q.dead&&(q.mode!=HARDCORE||q.health!=0))||(!q.dead&&q.health==0))return false;
    for(int i=0;i<14;i++){memcpy(&q.blocks[i],in+6+i*2,2);if(q.blocks[i]>999)return false;}
    memcpy(&q.berries,in+34,2);if(q.berries>999)return false;
    if((in[4]>>4)||in[60]||in[61]||in[62]||in[63]){
        for(int i=0;i<8;i++)q.hotbar[i]=(in[60+i/2]>>(4*(i%2)))&15;
        q.hotbar[8]=in[4]>>4;
        for(uint8_t id:q.hotbar)if(!id||id>13||id==9)return false;
    }
    memcpy(&q.x,in+36,8);memcpy(&q.y,in+44,8);memcpy(&q.z,in+52,8);
    // Inspect exponent bits first: fast-math can otherwise optimize away NaN checks.
    uint64_t xb,yb,zb;memcpy(&xb,&q.x,8);memcpy(&yb,&q.y,8);memcpy(&zb,&q.z,8);
    if(q.has_position&&(((xb>>52)&0x7ff)==0x7ff||((yb>>52)&0x7ff)==0x7ff||((zb>>52)&0x7ff)==0x7ff||
        q.x< -10000000||q.x>10000000||q.z< -10000000||q.z>10000000||q.y< -10||q.y>10000))return false;
    p=q;return true;
}
#endif
