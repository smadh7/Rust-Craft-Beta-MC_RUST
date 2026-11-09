#ifndef RUSTCRAFT_FRUSTUM_H
#define RUSTCRAFT_FRUSTUM_H
#include <cmath>
// A conservative sphere for a 16x64x16 chunk. Planes and camera basis are
// calculated once per frame, not for each chunk or rendering pass.
struct FrameFrustum {
    double px,py,pz,sy,cy,sp,cp,th,hmargin,vmargin;bool valid;
    FrameFrustum(double x,double y,double z,double yaw,double pitch,double aspect):px(x),py(y),pz(z),valid(aspect>0){
        const double rad=.017453292519943295;sy=std::sin(yaw*rad);cy=std::cos(yaw*rad);sp=std::sin(pitch*rad);cp=std::cos(pitch*rad);
        th=.7*aspect;hmargin=34.0*std::sqrt(1+th*th);vmargin=34.0*std::sqrt(1+.7*.7);
    }
    bool contains(int cx,int cz) const {
        if(!valid)return true;
        double dx=(double)cx*16+8-px,dy=32-py,dz=(double)cz*16+8-pz;
        double right=dx*cy-dz*sy,up=dx*sy*sp+dy*cp+dz*cy*sp,forward=-dx*sy*cp+dy*sp-dz*cy*cp;
        if(forward+34.0<.1||forward-34.0>180.0)return false;
        if(forward*th+hmargin<std::fabs(right))return false;
        if(forward*.7+vmargin<std::fabs(up))return false;
        return true;
    }
};
inline bool chunk_in_frustum(int cx,int cz,double px,double py,double pz,double yaw,double pitch,double aspect){return FrameFrustum(px,py,pz,yaw,pitch,aspect).contains(cx,cz);}
#endif
