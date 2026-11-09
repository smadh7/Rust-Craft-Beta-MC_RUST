#ifndef RUSTCRAFT_INTERACTION_H
#define RUSTCRAFT_INTERACTION_H
#include <cmath>
#include <cstdint>

struct RayHit { bool yes; int x,y,z,prevx,prevy,prevz; bool has_previous; };
struct ViewDirection { double x,y,z; };
inline ViewDirection view_direction(double yaw, double pitch) {
    const double radians = 0.017453292519943295;
    yaw *= radians; pitch *= radians;
    return {-std::sin(yaw)*std::cos(pitch), std::sin(pitch), -std::cos(yaw)*std::cos(pitch)};
}
// Starting inside a block can be broken, but cannot yield a placement cell.
template<class GetBlock>
RayHit trace_ray(double x,double y,double z,ViewDirection d,double reach,GetBlock get) {
    int ix=(int)std::floor(x), iy=(int)std::floor(y), iz=(int)std::floor(z);
    int ax=d.x>0?1:-1, ay=d.y>0?1:-1, az=d.z>0?1:-1;
    const double inf=1e100;
    double tx=d.x==0?inf:((d.x>0?ix+1:ix)-x)/d.x;
    double ty=d.y==0?inf:((d.y>0?iy+1:iy)-y)/d.y;
    double tz=d.z==0?inf:((d.z>0?iz+1:iz)-z)/d.z;
    double sx=d.x==0?inf:std::fabs(1/d.x), sy=d.y==0?inf:std::fabs(1/d.y), sz=d.z==0?inf:std::fabs(1/d.z);
    double t=0; int bx=ix,by=iy,bz=iz; bool previous=false;
    while(t<=reach) {
        uint8_t b=get(ix,iy,iz);
        if(b!=0 && b!=5) return {true,ix,iy,iz,bx,by,bz,previous};
        bx=ix;by=iy;bz=iz;previous=true;
        if(tx<=ty&&tx<=tz){t=tx;tx+=sx;ix+=ax;}
        else if(ty<=tz){t=ty;ty+=sy;iy+=ay;}
        else{t=tz;tz+=sz;iz+=az;}
    }
    return {false,0,0,0,0,0,0,false};
}
inline bool placement_clear(double px,double py,double pz,int x,int y,int z) {
    return !(px+.3>x&&px-.3<x+1&&py+1.8>y&&py<y+1&&pz+.3>z&&pz-.3<z+1);
}
#endif
