#ifndef RUSTCRAFT_WORLD_NAMES_H
#define RUSTCRAFT_WORLD_NAMES_H
#include <stdint.h>
#include <string>
#include <limits>

// Names are deliberately restricted to safe, portable filenames.
inline bool valid_world_name(const std::wstring& name) {
    if(name.empty() || name.size()>40 || name.front()==L' ' || name.back()==L' ')return false;
    std::wstring upper;
    for(wchar_t c:name){
        if(!((c>=L'a'&&c<=L'z')||(c>=L'A'&&c<=L'Z')||(c>=L'0'&&c<=L'9')||c==L' '||c==L'_'||c==L'-'))return false;
        upper.push_back(c>=L'a'&&c<=L'z'?c-(L'a'-L'A'):c);
    }
    if(upper==L"CON"||upper==L"PRN"||upper==L"AUX"||upper==L"NUL")return false;
    if(upper.size()==4&&(upper.substr(0,3)==L"COM"||upper.substr(0,3)==L"LPT")&&upper[3]>=L'1'&&upper[3]<=L'9')return false;
    return true;
}
inline bool parse_world_seed(const std::wstring& text,uint64_t& result){
    if(text.empty())return false;uint64_t n=0;
    for(wchar_t c:text){if(c<L'0'||c>L'9')return false;unsigned d=(unsigned)(c-L'0');if(n>(std::numeric_limits<uint64_t>::max()-d)/10)return false;n=n*10+d;}
    result=n;return true;
}
#endif
