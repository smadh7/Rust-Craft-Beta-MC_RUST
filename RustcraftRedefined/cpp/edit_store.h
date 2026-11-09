#ifndef RUSTCRAFT_EDIT_STORE_H
#define RUSTCRAFT_EDIT_STORE_H
#include <stdint.h>
#include <unordered_map>
#include <utility>
#include <cstring>

inline int32_t chunk_coord(int32_t v){int32_t q=v/16;return q-(v%16<0);}
inline uint32_t local_coord(int32_t v){return (uint32_t)((int64_t)v-(int64_t)chunk_coord(v)*16);}
inline uint64_t chunk_key(int32_t x,int32_t z){return ((uint64_t)(uint32_t)x<<32)|(uint32_t)z;}
inline int32_t signed_bits(uint32_t bits){int32_t v;std::memcpy(&v,&bits,sizeof v);return v;}
class EditStore {
    using Bucket=std::unordered_map<uint16_t,uint8_t>;
    std::unordered_map<uint64_t,Bucket> buckets;
    size_t count_=0;
public:
    size_t size() const {return count_;}
    void set(int32_t x,int32_t y,int32_t z,uint8_t id){
        if(y<0||y>=64||id>13)return;
        uint64_t key=chunk_key(chunk_coord(x),chunk_coord(z));
        uint16_t i=(uint16_t)((y*16+(int)local_coord(z))*16+(int)local_coord(x));
        auto inserted=buckets[key].emplace(i,id);if(inserted.second)count_++;else inserted.first->second=id;
    }
    void apply(int32_t cx,int32_t cz,uint8_t* blocks) const {
        auto it=buckets.find(chunk_key(cx,cz));if(it==buckets.end())return;
        for(const auto& edit:it->second)blocks[edit.first]=edit.second;
    }
    template<class F> void each(F f) const {
        for(const auto& chunk:buckets){int32_t cx=signed_bits((uint32_t)(chunk.first>>32)),cz=signed_bits((uint32_t)chunk.first);
            for(const auto& edit:chunk.second){int i=edit.first;int32_t x=(int32_t)((int64_t)cx*16+(i%16));int32_t z=(int32_t)((int64_t)cz*16+((i/16)%16));int32_t y=i/256;f(x,y,z,edit.second);}}
    }
    void swap(EditStore& other){buckets.swap(other.buckets);std::swap(count_,other.count_);}
};
#endif
