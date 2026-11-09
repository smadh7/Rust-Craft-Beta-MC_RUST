#include "../cpp/world_names.h"
#include "../cpp/world_format.h"
#include <cassert>
#include <cstdio>
int main(){
    assert(valid_world_name(L"My World"));assert(valid_world_name(L"A-1_2"));
    assert(!valid_world_name(L""));assert(!valid_world_name(L"../world"));assert(!valid_world_name(L"name."));
    assert(!valid_world_name(L" name"));assert(!valid_world_name(L"name "));assert(!valid_world_name(L"CON"));assert(!valid_world_name(L"lpt1"));
    assert(!valid_world_name(std::wstring(41,L'a')));
    uint64_t seed=0;assert(parse_world_seed(L"0",seed)&&seed==0);assert(parse_world_seed(L"12345",seed)&&seed==12345);
    assert(parse_world_seed(L"18446744073709551615",seed)&&seed==UINT64_MAX);
    assert(!parse_world_seed(L"18446744073709551616",seed));assert(!parse_world_seed(L"-1",seed));assert(!parse_world_seed(L"1a",seed));assert(!parse_world_seed(L"",seed));
    unsigned char header[20];uint32_t count=0;encode_world_header(header,UINT64_MAX,2);
    assert(decode_world_header(header,46,seed,count)&&seed==UINT64_MAX&&count==2);
    assert(!decode_world_header(header,45,seed,count));assert(!decode_world_header(header,47,seed,count));
    encode_world_header(header,12345,1000001);assert(!decode_world_header(header,20LL+13LL*1000001,seed,count));
    encode_world_header(header,12345,0);header[0]='X';assert(!decode_world_header(header,20,seed,count));
    puts("PASS world names, seed bounds, header format and damaged-save checks");
}
