#ifndef RUSTCRAFT_WORLD_FORMAT_H
#define RUSTCRAFT_WORLD_FORMAT_H
#include <stdint.h>
#include <string.h>

// Current format: magic, little-endian 64-bit seed, little-endian edit count,
// then 13 bytes per edit. Supported Windows builds are little-endian.
inline void encode_world_header(unsigned char (&header)[20],uint64_t seed,uint32_t count){
    memcpy(header,"RCRSAVE1",8);memcpy(header+8,&seed,8);memcpy(header+16,&count,4);
}
inline bool decode_world_header(const unsigned char (&header)[20],int64_t length,uint64_t& seed,uint32_t& count){
    if(memcmp(header,"RCRSAVE1",8)!=0)return false;
    memcpy(&seed,header+8,8);memcpy(&count,header+16,4);
    return count<=1000000&&length==20LL+13LL*count;
}
#endif
