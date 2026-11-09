#include "../cpp/gameplay.h"
#include "../cpp/world_format.h"
#include <cassert>
#include <cstdio>
#include <limits>
int main(){
    PlayerState p;assert(p.mode==CREATIVE);assert(consume_block(p,1));assert(harvest(p,1));assert(!harvest(p,9));assert(!damage(p,99));assert(p.health==20);
    p.mode=SURVIVAL;assert(!consume_block(p,1));assert(harvest(p,1));assert(p.blocks[1]==1);assert(consume_block(p,1));assert(p.blocks[1]==0);
    assert(!harvest(p,5));assert(!harvest(p,9));p.blocks[1]=999;assert(!harvest(p,1)&&p.blocks[1]==999);p.blocks[1]=0;assert(harvest(p,6));assert(craft_planks(p));assert(p.blocks[6]==0&&p.blocks[8]==4);
    p.berries=1;p.hunger=18;assert(eat_berries(p));assert(p.hunger==20&&p.berries==0);assert(!eat_berries(p));
    assert(!damage(p,4)&&p.health==16);assert(damage(p,100)&&p.health==0&&!p.dead);respawn(p);assert(p.health==20&&p.blocks[8]==0);
    p.mode=HARDCORE;assert(damage(p,20)&&p.dead);respawn(p);assert(p.dead);
    p.has_position=true;p.x=-32.5;p.y=60;p.z=44.5;p.selected=2;p.blocks[4]=15;p.berries=7;
    unsigned char data[64];encode_player(data,p);PlayerState q;assert(decode_player(data,q));assert(q.mode==HARDCORE&&q.dead&&q.x==-32.5&&q.blocks[4]==15&&q.berries==7);
    data[0]=99;assert(!decode_player(data,q));encode_player(data,p);data[4]=9;assert(!decode_player(data,q));
    p.dead=false;p.health=20;p.x=std::numeric_limits<double>::quiet_NaN();encode_player(data,p);assert(!decode_player(data,q));
    unsigned char header[20];uint64_t seed;uint32_t count;bool v2=false;encode_world_header(header,42,2,true);
    assert(decode_world_header(header,110,seed,count,&v2)&&v2&&seed==42&&count==2);assert(!decode_world_header(header,109,seed,count,&v2));
    encode_world_header(header,42,0);assert(decode_world_header(header,20,seed,count,&v2)&&!v2);
    puts("PASS mode rules, inventory, crafting, food, death, legacy and versioned saves");
}
