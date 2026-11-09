// Rustcraft Redefined: standalone offline Windows prototype.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <GL/gl.h>
#include <stdint.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include <unordered_map>
#include <string>
#include <algorithm>
#include "interaction.h"
#include "menu.h"
#include "world_format.h"

extern "C" void vc_generate_chunk(int32_t,int32_t,uint64_t,uint8_t*);
extern "C" int32_t vc_terrain_height(int32_t,int32_t,uint64_t);
extern "C" uint32_t vc_mesh_chunk(const uint8_t*,uint32_t*,uint32_t);
static uint64_t SEED=12345;
// Presets trade view distance and atmosphere for speed; no external shaders.
struct Preset { const wchar_t* name; int radius; bool gradient, clouds; };
static const Preset presets[]={{L"Potato / Basic",2,false,false},{L"Normal",3,true,false},{L"High",4,true,true},{L"Very High / Peak",5,true,true}};
static int graphics=1,skin=0;
static std::wstring world_name;
struct Pos { int x,y,z; bool operator==(const Pos& b)const{return x==b.x&&y==b.y&&z==b.z;} };
struct PosHash { size_t operator()(const Pos&p) const { return ((uint64_t)(uint32_t)p.x*0x9e3779b1u)^((uint64_t)(uint32_t)p.z*0x85ebca77u)^((uint64_t)(uint32_t)p.y*0xc2b2ae3du); } };
static std::unordered_map<Pos,uint8_t,PosHash> edits;
static std::wstring savepath;
static PlayerState player;
static bool legacy_needs_backup=false;
struct Chunk { int x,z; uint8_t b[16384]; GLuint opaque=0, trans=0; };
static std::vector<Chunk*> chunks;
static HWND window; static HDC dc; static HGLRC context;
static bool running=true,captured=false,fly=false,previous_space=false,previous_l=false,previous_r=false,previous_escape=false;
static int selected=0,wheel=0,width=1024,height=720;
static GLuint textures[15]={};
static const uint8_t hotbar[9]={1,2,3,4,6,8,10,12,13};
static double px=8,py=48,pz=8,vy=0; static float yaw=0,pitch=0; static double last_space=-5;
static int div16(int v){int q=v/16;return q-(v%16<0);} static int local16(int v){return v-div16(v)*16;}
static int idx(int x,int y,int z){return (y*16+z)*16+x;}
static int pidx(int x,int y,int z){return (y*18+z)*18+x;}
static Chunk* findchunk(int x,int z){for(auto c:chunks)if(c->x==x&&c->z==z)return c;return nullptr;}
static void populate(int x,int z,uint8_t* b){
    vc_generate_chunk(x,z,SEED,b);
    for(const auto &e:edits) if(e.first.y>=0&&e.first.y<64&&div16(e.first.x)==x&&div16(e.first.z)==z) b[idx(local16(e.first.x),e.first.y,local16(e.first.z))]=e.second;
}
static uint8_t block(int x,int y,int z){ if(y<0)return 9; if(y>=64)return 0; Chunk* c=findchunk(div16(x),div16(z)); return c?c->b[idx(local16(x),y,local16(z))]:0; }
static bool solid(uint8_t b){return b!=0&&b!=5;}
static void color(uint8_t b,int face){
    static const float colors[][3]={{0,0,0},{.48f,.49f,.50f},{.45f,.29f,.16f},{.31f,.61f,.20f},{.76f,.70f,.45f},{.19f,.42f,.75f},{.40f,.25f,.12f},{.20f,.48f,.18f},{.63f,.45f,.25f},{.18f,.18f,.20f},{.68f,.84f,.88f},{.92f,.94f,.96f},{.53f,.52f,.49f},{.62f,.27f,.20f}};
    if(b>13)b=1; float shade=face==3?1.f:(face==2?.55f:(face<2?.8f:.67f));
    glColor4f(colors[b][0]*shade,colors[b][1]*shade,colors[b][2]*shade,b==5?.55f:(b==10?.45f:1.f));
}
static void vertex(float x,float y,float z){glVertex3f(x,y,z);}
// Each independently repeated procedural texture can tile across greedy quads.
static void make_textures(){
    glGenTextures(15,textures);
    for(int id=1;id<15;id++){
        unsigned char pixels[16*16*4];
        for(int y=0;y<16;y++)for(int x=0;x<16;x++){
            uint32_t n=(uint32_t)(x*374761393u+y*668265263u+id*2246822519u); n=(n^(n>>13))*1274126177u; n^=n>>16;
            int v=200+(int)(n%50);
            if(id==1 || id==9 || id==12) v=170+(int)(n%75);
            if(id==4 || id==11) v=214+(int)(n%38);
            if(id==5) v=205+((x+y*2+(int)(n%4))%9)*5;
            if(id==6) v=(x%5==0?140:205)+(int)(n%28);
            if(id==7) v=160+(int)(n%90);
            if(id==8 && (y%5==0 || (x+(y/5%2)*8)%16==0)) v=135;
            if(id==13 && (y%8==0 || (x+(y/8%2)*8)%16==0)) v=125;
            if(id==14) v=y<5?185+(int)(n%65):135+(int)(n%60);
            int i=(y*16+x)*4;pixels[i]=pixels[i+1]=pixels[i+2]=(unsigned char)std::min(v,255);pixels[i+3]=255;
            if(id==14){int variation=(int)(n%24)-12;pixels[i]=(unsigned char)((y<5?91:139)+variation);pixels[i+1]=(unsigned char)((y<5?159:95)+variation);pixels[i+2]=(unsigned char)((y<5?58:55)+variation);}
        }
        glBindTexture(GL_TEXTURE_2D,textures[id]);glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_MIN_FILTER,GL_NEAREST);glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_MAG_FILTER,GL_NEAREST);
        glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_WRAP_S,GL_REPEAT);glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_WRAP_T,GL_REPEAT);
        glTexImage2D(GL_TEXTURE_2D,0,GL_RGBA,16,16,0,GL_RGBA,GL_UNSIGNED_BYTE,pixels);
    }
}
static void tv(float u,float v,float x,float y,float z){glTexCoord2f(u,v);vertex(x,y,z);}
static void make_list(GLuint list,const std::vector<uint32_t>& mesh,bool transparent){
    glNewList(list,GL_COMPILE);
    for(size_t i=0;i+1<mesh.size();i+=2){uint32_t a=mesh[i],b=mesh[i+1]; int x=a&31,y=(a>>5)&127,z=(a>>12)&31,face=(a>>17)&7,id=(a>>20)&255,w=b&127,h=(b>>7)&127;
        if((id==5||id==10)!=transparent)continue;
        if(id==3&&face!=3&&face!=2){float shade=face<2?.8f:.67f;glColor4f(shade,shade,shade,1);}else color((uint8_t)id,face);
        int tile=(id==3&&face!=3&&face!=2)?14:(id==3&&face==2?2:id);
        glBindTexture(GL_TEXTURE_2D,textures[tile]);glBegin(GL_QUADS);
        if(face<2){float X=(float)(x+(face==1));tv(0,h,X,y,z);tv(w,h,X,y,z+w);tv(w,0,X,y+h,z+w);tv(0,0,X,y+h,z);}
        else if(face<4){float Y=(float)(y+(face==3));tv(0,0,x,Y,z);tv(w,0,x+w,Y,z);tv(w,h,x+w,Y,z+h);tv(0,h,x,Y,z+h);}
        else {float Z=(float)(z+(face==5));tv(0,h,x,y,Z);tv(w,h,x+w,y,Z);tv(w,0,x+w,y+h,Z);tv(0,0,x,y+h,Z);}
        glEnd();
    }
    glEndList();
}
static void remesh(Chunk* c){
    std::vector<uint8_t> pad(18*18*66,0), side(16384);
    for(int y=0;y<64;y++)for(int z=0;z<16;z++)for(int x=0;x<16;x++)pad[pidx(x+1,y+1,z+1)]=c->b[idx(x,y,z)];
    for(int d=0;d<4;d++) {int nx=c->x+(d==0?-1:d==1?1:0), nz=c->z+(d==2?-1:d==3?1:0); populate(nx,nz,side.data());
        for(int y=0;y<64;y++)for(int t=0;t<16;t++) { if(d==0)pad[pidx(0,y+1,t+1)]=side[idx(15,y,t)]; if(d==1)pad[pidx(17,y+1,t+1)]=side[idx(0,y,t)]; if(d==2)pad[pidx(t+1,y+1,0)]=side[idx(t,y,15)]; if(d==3)pad[pidx(t+1,y+1,17)]=side[idx(t,y,0)]; }
    }
    uint32_t n=vc_mesh_chunk(pad.data(),nullptr,0); std::vector<uint32_t> q(n); vc_mesh_chunk(pad.data(),q.data(),n);
    if(!c->opaque)c->opaque=glGenLists(1); if(!c->trans)c->trans=glGenLists(1); make_list(c->opaque,q,false);make_list(c->trans,q,true);
}
static void stream(){int cx=div16((int)floor(px)),cz=div16((int)floor(pz));int radius=presets[graphics].radius;
    for(size_t i=0;i<chunks.size();) {Chunk*c=chunks[i]; if(abs(c->x-cx)>radius+1||abs(c->z-cz)>radius+1){glDeleteLists(c->opaque,1);glDeleteLists(c->trans,1);delete c;chunks.erase(chunks.begin()+i);} else i++;}
    for(int r=0;r<=radius;r++)for(int z=-r;z<=r;z++)for(int x=-r;x<=r;x++){ if(abs(x)!=r&&abs(z)!=r)continue; if(!findchunk(cx+x,cz+z)){Chunk*c=new Chunk; c->x=cx+x;c->z=cz+z;populate(c->x,c->z,c->b);chunks.push_back(c); remesh(c);return;} }
}
// Refuse damaged or incomplete files instead of overwriting them on exit.
static bool load_world(){
    if(savepath.empty())return true;
    FILE*f=_wfopen(savepath.c_str(),L"rb");if(!f)return false;
    unsigned char header[20];uint64_t seed;uint32_t count;bool v2=false;
    bool ok=_fseeki64(f,0,SEEK_END)==0;int64_t length=ok?_ftelli64(f):-1;if(ok)ok=_fseeki64(f,0,SEEK_SET)==0;
    ok=ok&&fread(header,1,20,f)==20&&decode_world_header(header,length,seed,count,&v2)&&seed==SEED;
    PlayerState loaded_player;
    if(ok&&v2){unsigned char state[64];ok=fread(state,1,64,f)==64&&decode_player(state,loaded_player)&&!loaded_player.dead;}
    std::unordered_map<Pos,uint8_t,PosHash> loaded;
    if(ok)for(uint32_t i=0;i<count;i++){int32_t v[3];uint8_t b;if(fread(v,4,3,f)!=3||fread(&b,1,1,f)!=1||v[1]<0||v[1]>=64||b>13){ok=false;break;}loaded[{v[0],v[1],v[2]}]=b;}
    if(ok&&fgetc(f)!=EOF)ok=false;if(ferror(f))ok=false;fclose(f);
    if(ok){edits.swap(loaded);player=loaded_player;legacy_needs_backup=!v2;}return ok;
}
static bool save(){
    if(savepath.empty())return true;
    if(edits.size()>1000000)return false;
    if(legacy_needs_backup){std::wstring backup=savepath+L".v1-backup";
        if(GetFileAttributesW(backup.c_str())==INVALID_FILE_ATTRIBUTES&&!CopyFileW(savepath.c_str(),backup.c_str(),TRUE))return false;
        legacy_needs_backup=false;
    }
    std::wstring tmp=savepath+L".tmp";FILE*f=_wfopen(tmp.c_str(),L"wb");if(!f)return false;
    player.selected=(uint8_t)selected;player.has_position=true;player.x=px;player.y=py;player.z=pz;
    unsigned char header[20],state[64];encode_world_header(header,SEED,(uint32_t)edits.size(),true);encode_player(state,player);
    bool ok=fwrite(header,1,20,f)==20&&fwrite(state,1,64,f)==64;
    for(const auto&e:edits){int32_t v[3]={e.first.x,e.first.y,e.first.z};ok=ok&&fwrite(v,4,3,f)==3&&fwrite(&e.second,1,1,f)==1;}
    if(fclose(f)!=0)ok=false;if(!ok){DeleteFileW(tmp.c_str());return false;}
    return MoveFileExW(tmp.c_str(),savepath.c_str(),MOVEFILE_REPLACE_EXISTING)!=0;
}
static void save_with_warning(){static bool warned=false;if(!save()&&!warned){warned=true;MessageBoxW(window,L"The world could not be saved. Your latest edits may be lost. Check that your user data folder is writable.",L"Save error",MB_OK|MB_ICONWARNING);}}
static void setblock(int x,int y,int z,uint8_t id){if(y<=0||y>=64)return;Chunk*c=findchunk(div16(x),div16(z));if(!c)return;c->b[idx(local16(x),y,local16(z))]=id;edits[{x,y,z}]=id;remesh(c); if(local16(x)==0){Chunk*n=findchunk(c->x-1,c->z);if(n)remesh(n);} if(local16(x)==15){Chunk*n=findchunk(c->x+1,c->z);if(n)remesh(n);} if(local16(z)==0){Chunk*n=findchunk(c->x,c->z-1);if(n)remesh(n);} if(local16(z)==15){Chunk*n=findchunk(c->x,c->z+1);if(n)remesh(n);}}
static bool collides(double x,double y,double z){for(int X=(int)floor(x-.3);X<=(int)floor(x+.3);X++)for(int Y=(int)floor(y);Y<=(int)floor(y+1.79);Y++)for(int Z=(int)floor(z-.3);Z<=(int)floor(z+.3);Z++)if((Y>=0&&Y<64&&!findchunk(div16(X),div16(Z)))||solid(block(X,Y,Z)))return true;return false;}
static RayHit ray(){return trace_ray(px,py+1.62,pz,view_direction(yaw,pitch),6.0,[](int x,int y,int z){return block(x,y,z);});}
static void capture(bool on){captured=on; if(on){SetCapture(window);RECT r;GetClientRect(window,&r);POINT p={(r.right-r.left)/2,(r.bottom-r.top)/2};ClientToScreen(window,&p);SetCursorPos(p.x,p.y);while(ShowCursor(FALSE)>=0){} }else{ReleaseCapture();while(ShowCursor(TRUE)<0){}}}
static LRESULT CALLBACK proc(HWND h,UINT msg,WPARAM w,LPARAM l){switch(msg){case WM_CLOSE:running=false;return 0;case WM_DESTROY:running=false;return 0;case WM_SIZE:width=LOWORD(l);height=HIWORD(l);return 0;case WM_MOUSEWHEEL:wheel+=(GET_WHEEL_DELTA_WPARAM(w)>0?1:-1);return 0;case WM_LBUTTONDOWN:if(!captured){capture(true);previous_l=true;}return 0;case WM_KILLFOCUS:if(captured)capture(false);return 0;}return DefWindowProcW(h,msg,w,l);}
static bool key(int k){return (GetAsyncKeyState(k)&0x8000)!=0;}
static void spawn_player(){
    uint8_t terrain[16384];populate(0,0,terrain);int top=0;
    for(int y=63;y>=0;y--)if(solid(terrain[idx(8,y,8)])){top=y;break;}
    px=8.5;pz=8.5;py=top+1.01;vy=0;
}
static void take_damage(int amount){
    if(!limited(player)||amount<=0)return;
    bool died=damage(player,amount);
    if(died){
        if(player.mode==HARDCORE){save_with_warning();running=false;if(captured)capture(false);MessageBoxW(window,L"Your Hardcore run has ended. This world is kept but cannot be played again.",L"Hardcore",MB_OK|MB_ICONINFORMATION);return;}
        respawn(player);spawn_player();fly=false;save_with_warning();if(captured)capture(false);MessageBoxW(window,L"You died and returned to spawn. Your collected items were lost.",L"Survival",MB_OK|MB_ICONINFORMATION);
    } else save_with_warning();
}
static void input(double dt,double now){
    static bool prev_c=false,prev_f=false;static double mining=0,exhaustion=0,starve=0,heal=0,autosave=0,air=15,drown=0;
    static int bx=0,by=0,bz=0;
    bool esc=key(VK_ESCAPE);if(esc&&!previous_escape&&captured)capture(false);previous_escape=esc;
    for(int i=0;i<4;i++)if(key(VK_F1+i))graphics=i;
    if(!captured){mining=0;return;}
    RECT r;GetClientRect(window,&r);POINT p={(r.right-r.left)/2,(r.bottom-r.top)/2};ClientToScreen(window,&p);POINT cur;GetCursorPos(&cur);yaw-=(cur.x-p.x)*.12f;pitch-=(cur.y-p.y)*.12f;pitch=std::max(-89.f,std::min(89.f,pitch));SetCursorPos(p.x,p.y);
    bool space=key(VK_SPACE);bool ground=collides(px,py-.06,pz);
    if(space&&!previous_space){if(!limited(player)&&now-last_space<.3){fly=!fly;vy=0;}else if(ground&&!fly)vy=8;last_space=now;}previous_space=space;
    for(int i=0;i<9;i++)if(key('1'+i))selected=i;if(wheel){selected=(selected-wheel%9+9)%9;wheel=0;}
    bool c=key('C'),f=key('F');if(c&&!prev_c&&craft_planks(player))save_with_warning();if(f&&!prev_f&&eat_berries(player))save_with_warning();prev_c=c;prev_f=f;
    double forward=(key('W')?1:0)-(key('S')?1:0),side=(key('D')?1:0)-(key('A')?1:0),len=sqrt(forward*forward+side*side);if(len>0){forward/=len;side/=len;}
    bool in_water=block((int)floor(px),(int)floor(py+1.0),(int)floor(pz))==5;
    bool underwater=block((int)floor(px),(int)floor(py+1.62),(int)floor(pz))==5;
    bool sprint=key(VK_CONTROL)&&(!limited(player)||player.hunger>6);double angle=yaw*.0174532925, speed=(sprint?8:5)*dt*(in_water?.55:1);double dx=(-sin(angle)*forward+cos(angle)*side)*speed,dz=(-cos(angle)*forward-sin(angle)*side)*speed;
    if(!collides(px+dx,py,pz))px+=dx;if(!collides(px,py,pz+dz))pz+=dz;
    if(fly)vy=((space?1:0)-(key(VK_SHIFT)?1:0))*7;
    else if(in_water){vy=std::max(-3.0,vy-5*dt);if(space)vy=4;if(key(VK_SHIFT))vy=-4;}
    else vy=std::max(-30.0,vy-22*dt);double dy=vy*dt;int steps=std::max(1,(int)ceil(fabs(dy)/.1));
    for(int i=0;i<steps;i++){if(!collides(px,py+dy/steps,pz))py+=dy/steps;else{double impact=vy;vy=0;if(dy<0&&impact<-12){take_damage((int)((-impact-12)/3)+1);if(!running||!captured)return;}break;}}
    if(limited(player)){
        if(underwater){air=std::max(0.0,air-dt);if(air<=0){drown+=dt;if(drown>=2){drown=0;take_damage(2);if(!running||!captured)return;}}}else{air=15;drown=0;}
        exhaustion+=dt*(len>0?(sprint?.08:.02):.002);if(exhaustion>=1){exhaustion-=1;if(player.hunger)player.hunger--;}
        if(player.hunger==0){starve+=dt;if(starve>=4){starve=0;take_damage(1);if(!running||!captured)return;}}else starve=0;
        if(player.hunger>=18&&player.health<20){heal+=dt;if(heal>=8){heal=0;player.health++;player.hunger--;}}else heal=0;
        autosave+=dt;if(autosave>=15){autosave=0;save_with_warning();}
    }
    RayHit h=ray();bool l=key(VK_LBUTTON),rr=key(VK_RBUTTON);
    if(l&&h.yes&&h.y>0&&h.y<64&&findchunk(div16(h.x),div16(h.z))){
        if(!previous_l||h.x!=bx||h.y!=by||h.z!=bz){mining=0;bx=h.x;by=h.y;bz=h.z;}
        mining+=dt;uint8_t b=block(h.x,h.y,h.z);
        if((!limited(player)&&!previous_l)||(limited(player)&&mining>=.65)){
            if(harvest(player,b)){if(limited(player)&&b==7&&player.berries<999)player.berries++;setblock(h.x,h.y,h.z,0);save_with_warning();}mining=0;
        }
    } else mining=0;
    if(h.yes&&h.has_previous&&rr&&!previous_r){int x=h.prevx,y=h.prevy,z=h.prevz;uint8_t target=block(x,y,z);
        if(y>0&&y<64&&findchunk(div16(x),div16(z))&&(target==0||target==5)&&placement_clear(px,py,pz,x,y,z)&&consume_block(player,hotbar[selected])){setblock(x,y,z,hotbar[selected]);save_with_warning();}}
    previous_l=l;previous_r=rr;
}
static void sky(){
    // Original gradient and sun, drawn before depth-tested world geometry.
    glDisable(GL_DEPTH_TEST);glDisable(GL_FOG);glDisable(GL_TEXTURE_2D);glDisable(GL_BLEND);
    glMatrixMode(GL_PROJECTION);glLoadIdentity();glOrtho(0,width,height,0,-1,1);glMatrixMode(GL_MODELVIEW);glLoadIdentity();
    float horizon=(float)std::max(0.0,std::min((double)height,height*.52+pitch*height/100.0));
    glBegin(GL_QUADS);
    if(presets[graphics].gradient){
        glColor3f(.25f,.48f,.76f);vertex(0,0,0);vertex(width,0,0);
        glColor3f(.75f,.82f,.83f);vertex(width,horizon,0);vertex(0,horizon,0);
        glColor3f(.75f,.82f,.83f);vertex(0,horizon,0);vertex(width,horizon,0);
        glColor3f(.57f,.71f,.78f);vertex(width,height,0);vertex(0,height,0);
    } else {glColor3f(.52f,.72f,.91f);vertex(0,0,0);vertex(width,0,0);vertex(width,height,0);vertex(0,height,0);}
    glEnd();
    if(!presets[graphics].gradient)return;
    glMatrixMode(GL_PROJECTION);glLoadIdentity();double zn=.1,top=zn*.7,right=top*width/height;glFrustum(-right,right,-top,top,zn,180);
    glMatrixMode(GL_MODELVIEW);glLoadIdentity();glRotatef(-pitch,1,0,0);glRotatef(-yaw,0,1,0);
    glEnable(GL_BLEND);glBlendFunc(GL_SRC_ALPHA,GL_ONE_MINUS_SRC_ALPHA);
    // A soft translucent halo and an opaque sun disc, not a post-process shader.
    for(int ring=0;ring<2;ring++){float radius=ring==0?8.5f:3.0f;glBegin(GL_TRIANGLE_FAN);glColor4f(1.f,.94f,.76f,ring==0?.16f:1.f);vertex(12,24,-70);
        for(int i=0;i<=32;i++){float a=(float)(i*6.28318530718/32);glColor4f(1.f,.94f,.76f,ring==0?0.f:1.f);vertex(12+cosf(a)*radius,24+sinf(a)*radius,-70);}glEnd();}
    glDisable(GL_BLEND);
}
static void clouds(){
    if(!presets[graphics].clouds)return;
    glDisable(GL_TEXTURE_2D);glEnable(GL_BLEND);glBlendFunc(GL_SRC_ALPHA,GL_ONE_MINUS_SRC_ALPHA);glDepthMask(GL_FALSE);
    int cx=(int)floor(px/24),cz=(int)floor(pz/24);
    glColor4f(.94f,.96f,1.f,.72f);glBegin(GL_QUADS);
    for(int z=-4;z<=4;z++)for(int x=-4;x<=4;x++){
        uint32_t h=(uint32_t)(cx+x)*374761393u+(uint32_t)(cz+z)*668265263u;h^=h>>13;h*=1274126177u;
        if((h&7)<3)continue;float X=(float)((cx+x)*24),Z=(float)((cz+z)*24),Y=62.f;
        vertex(X,Y,Z);vertex(X+18,Y,Z);vertex(X+18,Y,Z+12);vertex(X,Y,Z+12);
    }
    glEnd();glDepthMask(GL_TRUE);glDisable(GL_BLEND);
}
static void box2(float x,float y,float w,float h){vertex(x,y,0);vertex(x+w,y,0);vertex(x+w,y+h,0);vertex(x,y+h,0);}
static void draw_number(int value,float x,float y){
    static const unsigned char masks[]={63,6,91,79,102,109,125,7,127,111};
    char buf[4];snprintf(buf,sizeof buf,"%d",std::min(value,999));
    glColor3f(1,1,1);glBegin(GL_QUADS);
    for(int i=0;buf[i];i++){float X=x+i*7;unsigned m=masks[buf[i]-'0'];
        if(m&1)box2(X+1,y,4,1);if(m&2)box2(X+5,y+1,1,4);if(m&4)box2(X+5,y+6,1,4);
        if(m&8)box2(X+1,y+10,4,1);if(m&16)box2(X,y+6,1,4);if(m&32)box2(X,y+1,1,4);if(m&64)box2(X+1,y+5,4,1);
    }glEnd();
}
static void render(){
    if(width<1||height<1)return;
    glViewport(0,0,width,height);glClearColor(.52f,.72f,.91f,1);glClear(GL_COLOR_BUFFER_BIT|GL_DEPTH_BUFFER_BIT);
    sky();glEnable(GL_DEPTH_TEST);glEnable(GL_FOG);
    GLfloat fog[]={presets[graphics].gradient?.70f:.52f,presets[graphics].gradient?.79f:.72f,presets[graphics].gradient?.84f:.91f,1};
    glFogfv(GL_FOG_COLOR,fog);glFogi(GL_FOG_MODE,GL_LINEAR);float fog_end=(presets[graphics].radius+.4f)*16.f;glFogf(GL_FOG_START,fog_end*.52f);glFogf(GL_FOG_END,fog_end);
    glMatrixMode(GL_PROJECTION);glLoadIdentity();double znear=.1,top=znear*.7,right=top*width/height;glFrustum(-right,right,-top,top,znear,180);
    glMatrixMode(GL_MODELVIEW);glLoadIdentity();glRotatef(-pitch,1,0,0);glRotatef(-yaw,0,1,0);glTranslated(-px,-py-1.62,-pz);
    clouds();glEnable(GL_TEXTURE_2D);glTexEnvi(GL_TEXTURE_ENV,GL_TEXTURE_ENV_MODE,GL_MODULATE);
    glDisable(GL_BLEND);for(auto c:chunks){glPushMatrix();glTranslatef((float)(c->x*16),0,(float)(c->z*16));glCallList(c->opaque);glPopMatrix();}
    // Sort chunks back-to-front for translucent surfaces; faces within a chunk are not sorted.
    std::vector<Chunk*> ordered(chunks);std::sort(ordered.begin(),ordered.end(),[](const Chunk*a,const Chunk*b){double ax=a->x*16+8-px,az=a->z*16+8-pz,bx=b->x*16+8-px,bz=b->z*16+8-pz;return ax*ax+az*az>bx*bx+bz*bz;});
    glEnable(GL_BLEND);glBlendFunc(GL_SRC_ALPHA,GL_ONE_MINUS_SRC_ALPHA);glDepthMask(GL_FALSE);for(auto c:ordered){glPushMatrix();glTranslatef((float)(c->x*16),0,(float)(c->z*16));glCallList(c->trans);glPopMatrix();}glDepthMask(GL_TRUE);glDisable(GL_BLEND);glDisable(GL_TEXTURE_2D);
    RayHit h=ray();if(h.yes){glDisable(GL_FOG);glColor3f(0,0,0);float x=(float)h.x-.003f,y=(float)h.y-.003f,z=(float)h.z-.003f,s=1.006f;glBegin(GL_LINES);for(int a=0;a<2;a++)for(int b=0;b<2;b++){vertex(x,y+a*s,z+b*s);vertex(x+s,y+a*s,z+b*s);vertex(x+a*s,y,z+b*s);vertex(x+a*s,y+s,z+b*s);vertex(x+a*s,y+b*s,z);vertex(x+a*s,y+b*s,z+s);}glEnd();}
    glDisable(GL_FOG);glDisable(GL_DEPTH_TEST);glMatrixMode(GL_PROJECTION);glLoadIdentity();glOrtho(0,width,height,0,-1,1);glMatrixMode(GL_MODELVIEW);glLoadIdentity();glColor3f(1,1,1);glBegin(GL_LINES);vertex(width/2-7,height/2,0);vertex(width/2+7,height/2,0);vertex(width/2,height/2-7,0);vertex(width/2,height/2+7,0);glEnd();
    // A simple first-person arm makes the chosen skin visible without a player model.
    if(width>=700&&height>=450){
        static const float hands[4][3]={{.76f,.51f,.37f},{.37f,.23f,.17f},{.93f,.73f,.58f},{.66f,.56f,.49f}};
        static const float sleeves[4][3]={{.20f,.41f,.48f},{.43f,.31f,.20f},{.30f,.43f,.30f},{.31f,.29f,.48f}};
        float x=(float)width-145,y=(float)height-150;glColor3fv(sleeves[skin]);glBegin(GL_QUADS);vertex(x+38,y+75,0);vertex(x+125,y+75,0);vertex(x+145,y+150,0);vertex(x+38,y+150,0);glEnd();
        glColor3fv(hands[skin]);glBegin(GL_QUADS);vertex(x+35,y+24,0);vertex(x+118,y+24,0);vertex(x+125,y+80,0);vertex(x+38,y+80,0);glEnd();
    }
    if(limited(player)){
        for(int i=0;i<20;i++){
            float x=width/2-162.f+i*8,y=height-64.f;glColor3f(i<player.health?.85f:.20f,.08f,.08f);glBegin(GL_QUADS);box2(x,y,6,9);glEnd();
            x=width/2+4.f+i*8;glColor3f(i<player.hunger?.93f:.20f,i<player.hunger?.64f:.18f,.12f);glBegin(GL_QUADS);box2(x,y,6,9);glEnd();
        }
    }
    for(int i=0;i<9;i++){float x=width/2-162.f+i*36,y=height-45.f;glColor3f(i==selected?1:.15f,i==selected?1:.15f,i==selected?1:.15f);glBegin(GL_QUADS);vertex(x,y,0);vertex(x+34,y,0);vertex(x+34,y+34,0);vertex(x,y+34,0);glEnd();color(hotbar[i],3);glBegin(GL_QUADS);vertex(x+4,y+4,0);vertex(x+30,y+4,0);vertex(x+30,y+30,0);vertex(x+4,y+30,0);glEnd();if(limited(player))draw_number(player.blocks[hotbar[i]],x+5,y+20);}SwapBuffers(dc);
}
int WINAPI WinMain(HINSTANCE instance,HINSTANCE,LPSTR,int show){SetProcessDPIAware();
    MenuResult menu=open_start_menu(instance);if(!menu.start)return 0;
    SEED=menu.seed;graphics=menu.graphics;skin=menu.skin;savepath=menu.save_path;world_name=menu.world_name;player.mode=menu.mode;
    if(!load_world()){MessageBoxW(nullptr,L"The selected world is incomplete or damaged and was not modified.",L"Unable to open world",MB_OK|MB_ICONERROR);return 1;}
    if(player.has_position){px=player.x;py=player.y;pz=player.z;}else spawn_player();selected=player.selected;
    WNDCLASSW wc={};wc.style=CS_OWNDC;wc.lpfnWndProc=proc;wc.hInstance=instance;wc.lpszClassName=L"RustcraftRedefinedWindow";wc.hCursor=LoadCursor(nullptr,IDC_ARROW);if(!RegisterClassW(&wc))return 1;window=CreateWindowW(wc.lpszClassName,L"Rustcraft Redefined",WS_OVERLAPPEDWINDOW,CW_USEDEFAULT,CW_USEDEFAULT,1024,720,nullptr,nullptr,instance,nullptr);if(!window)return 1;dc=GetDC(window);PIXELFORMATDESCRIPTOR pfd={};pfd.nSize=sizeof(pfd);pfd.nVersion=1;pfd.dwFlags=PFD_DRAW_TO_WINDOW|PFD_SUPPORT_OPENGL|PFD_DOUBLEBUFFER;pfd.iPixelType=PFD_TYPE_RGBA;pfd.cColorBits=32;pfd.cDepthBits=24;int pf=ChoosePixelFormat(dc,&pfd);if(!pf||!SetPixelFormat(dc,pf,&pfd))return 1;context=wglCreateContext(dc);if(!context||!wglMakeCurrent(dc,context))return 1;ShowWindow(window,show);make_textures();LARGE_INTEGER freq,prev;QueryPerformanceFrequency(&freq);QueryPerformanceCounter(&prev);double now=0,title=0;int frames=0;
    while(running){MSG msg;while(PeekMessageW(&msg,nullptr,0,0,PM_REMOVE)){if(msg.message==WM_QUIT)running=false;TranslateMessage(&msg);DispatchMessageW(&msg);}LARGE_INTEGER t;QueryPerformanceCounter(&t);double dt=(double)(t.QuadPart-prev.QuadPart)/freq.QuadPart;prev=t;double elapsed=dt;if(dt>.05)dt=.05;now+=elapsed;stream();input(dt,now);render();frames++;title+=elapsed;if(title>=1){wchar_t text[256];swprintf(text,256,L"Rustcraft Redefined | %ls | %ls | HP %u Food %u Berries %u Slot %u | FPS %d | %.1f %.1f %.1f | seed %llu | %ls",world_name.c_str(),player.mode==CREATIVE?L"Creative":(player.mode==SURVIVAL?L"Survival":L"Hardcore"),(unsigned)player.health,(unsigned)player.hunger,(unsigned)player.berries,(unsigned)(limited(player)?player.blocks[hotbar[selected]]:0),(int)(frames/title),px,py,pz,(unsigned long long)SEED,presets[graphics].name);SetWindowTextW(window,text);title=0;frames=0;}Sleep(1);}
    if(captured)capture(false);save_with_warning();menu.graphics=graphics;save_preferences(menu);for(auto c:chunks){glDeleteLists(c->opaque,1);glDeleteLists(c->trans,1);delete c;}glDeleteTextures(15,textures);wglMakeCurrent(nullptr,nullptr);wglDeleteContext(context);ReleaseDC(window,dc);DestroyWindow(window);return 0;
}
