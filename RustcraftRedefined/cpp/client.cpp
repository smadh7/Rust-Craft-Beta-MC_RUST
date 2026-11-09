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

extern "C" void vc_generate_chunk(int32_t,int32_t,uint64_t,uint8_t*);
extern "C" uint32_t vc_mesh_chunk(const uint8_t*,uint32_t*,uint32_t);
static const uint64_t SEED=12345;
static const int RADIUS=3;
struct Pos { int x,y,z; bool operator==(const Pos& b)const{return x==b.x&&y==b.y&&z==b.z;} };
struct PosHash { size_t operator()(const Pos&p) const { return ((uint64_t)(uint32_t)p.x*0x9e3779b1u)^((uint64_t)(uint32_t)p.z*0x85ebca77u)^((uint64_t)(uint32_t)p.y*0xc2b2ae3du); } };
static std::unordered_map<Pos,uint8_t,PosHash> edits;
static std::wstring savepath;
struct Chunk { int x,z; uint8_t b[16384]; GLuint opaque=0, trans=0; };
static std::vector<Chunk*> chunks;
static HWND window; static HDC dc; static HGLRC context;
static bool running=true,captured=false,fly=false,previous_space=false,previous_l=false,previous_r=false,previous_escape=false;
static int selected=0,wheel=0,width=1024,height=720;
static const uint8_t hotbar[9]={1,2,3,4,7,8,10,12,13};
static double px=8,py=48,pz=8,vy=0; static float yaw=0,pitch=0; static double last_space=-5;
static int div16(int v){return v>=0?v/16:-((15-v)/16);} static int local16(int v){return v-div16(v)*16;}
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
static void make_list(GLuint list,const std::vector<uint32_t>& mesh,bool transparent){
    glNewList(list,GL_COMPILE); glBegin(GL_QUADS);
    for(size_t i=0;i+1<mesh.size();i+=2){uint32_t a=mesh[i],b=mesh[i+1]; int x=a&31,y=(a>>5)&127,z=(a>>12)&31,face=(a>>17)&7,id=(a>>20)&255,w=b&127,h=(b>>7)&127;
        if((id==5||id==10)!=transparent)continue; color((uint8_t)id,face);
        // Face culling is disabled; winding is not relevant.
        if(face<2){float X=(float)(x+(face==1));vertex(X,y,z);vertex(X,y,z+w);vertex(X,y+h,z+w);vertex(X,y+h,z);}
        else if(face<4){float Y=(float)(y+(face==3));vertex(x,Y,z);vertex(x+w,Y,z);vertex(x+w,Y,z+h);vertex(x,Y,z+h);}
        else {float Z=(float)(z+(face==5));vertex(x,y,Z);vertex(x+w,y,Z);vertex(x+w,y+h,Z);vertex(x,y+h,Z);}
    }
    glEnd();glEndList();
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
static void stream(){int cx=div16((int)floor(px)),cz=div16((int)floor(pz));
    for(size_t i=0;i<chunks.size();) {Chunk*c=chunks[i]; if(abs(c->x-cx)>RADIUS+1||abs(c->z-cz)>RADIUS+1){glDeleteLists(c->opaque,1);glDeleteLists(c->trans,1);delete c;chunks.erase(chunks.begin()+i);} else i++;}
    for(int r=0;r<=RADIUS;r++)for(int z=-r;z<=r;z++)for(int x=-r;x<=r;x++){ if(abs(x)!=r&&abs(z)!=r)continue; if(!findchunk(cx+x,cz+z)){Chunk*c=new Chunk; c->x=cx+x;c->z=cz+z;populate(c->x,c->z,c->b);chunks.push_back(c); remesh(c);return;} }
}
static void init_save(){wchar_t base[32768];DWORD n=GetEnvironmentVariableW(L"APPDATA",base,32768); if(!n||n>=32768){n=GetEnvironmentVariableW(L"USERPROFILE",base,32768);if(!n||n>=32768)return;}
    std::wstring dir=std::wstring(base)+L"\\Rustcraft Redefined";if(!CreateDirectoryW(dir.c_str(),nullptr)&&GetLastError()!=ERROR_ALREADY_EXISTS)return;savepath=dir+L"\\world.bin";
    FILE*f=_wfopen(savepath.c_str(),L"rb");if(!f)return; char magic[8];uint64_t seed;uint32_t count;
    if(fread(magic,1,8,f)==8&&!memcmp(magic,"RCRSAVE1",8)&&fread(&seed,8,1,f)==1&&seed==SEED&&fread(&count,4,1,f)==1&&count<=1000000){for(uint32_t i=0;i<count;i++){int32_t v[3];uint8_t b;if(fread(v,4,3,f)!=3||fread(&b,1,1,f)!=1)break;if(v[1]>=0&&v[1]<64&&b<=13)edits[{v[0],v[1],v[2]}]=b;}}fclose(f);
}
static void save(){if(savepath.empty())return; std::wstring tmp=savepath+L".tmp";FILE*f=_wfopen(tmp.c_str(),L"wb");if(!f)return;uint32_t count=(uint32_t)edits.size();bool ok=fwrite("RCRSAVE1",1,8,f)==8&&fwrite(&SEED,8,1,f)==1&&fwrite(&count,4,1,f)==1;
    for(const auto&e:edits){int32_t v[3]={e.first.x,e.first.y,e.first.z};ok=ok&&fwrite(v,4,3,f)==3&&fwrite(&e.second,1,1,f)==1;} if(fclose(f)!=0)ok=false;if(ok)MoveFileExW(tmp.c_str(),savepath.c_str(),MOVEFILE_REPLACE_EXISTING);else DeleteFileW(tmp.c_str());
}
static void setblock(int x,int y,int z,uint8_t id){if(y<=0||y>=64)return;Chunk*c=findchunk(div16(x),div16(z));if(!c)return;c->b[idx(local16(x),y,local16(z))]=id;edits[{x,y,z}]=id;remesh(c); if(local16(x)==0){Chunk*n=findchunk(c->x-1,c->z);if(n)remesh(n);} if(local16(x)==15){Chunk*n=findchunk(c->x+1,c->z);if(n)remesh(n);} if(local16(z)==0){Chunk*n=findchunk(c->x,c->z-1);if(n)remesh(n);} if(local16(z)==15){Chunk*n=findchunk(c->x,c->z+1);if(n)remesh(n);}save();}
static bool collides(double x,double y,double z){for(int X=(int)floor(x-.3);X<=(int)floor(x+.3);X++)for(int Y=(int)floor(y);Y<=(int)floor(y+1.79);Y++)for(int Z=(int)floor(z-.3);Z<=(int)floor(z+.3);Z++)if(solid(block(X,Y,Z)))return true;return false;}
struct Hit {bool yes;int x,y,z,prevx,prevy,prevz;};
static Hit ray(){float yr=yaw*0.0174532925f, pr=pitch*0.0174532925f; double dx=-sinf(yr)*cosf(pr),dy=sinf(pr),dz=-cosf(yr)*cosf(pr);
    double x=px,y=py+1.62,z=pz;int ix=(int)floor(x),iy=(int)floor(y),iz=(int)floor(z), ax=dx>0?1:-1,ay=dy>0?1:-1,az=dz>0?1:-1;
    double tx=dx==0?1e30:((dx>0?ix+1:ix)-x)/dx,ty=dy==0?1e30:((dy>0?iy+1:iy)-y)/dy,tz=dz==0?1e30:((dz>0?iz+1:iz)-z)/dz, sx=dx==0?1e30:fabs(1/dx),sy=dy==0?1e30:fabs(1/dy),sz=dz==0?1e30:fabs(1/dz),t=0;int bx=ix,by=iy,bz=iz;
    while(t<=6){if(block(ix,iy,iz)!=0&&block(ix,iy,iz)!=5)return {true,ix,iy,iz,bx,by,bz};bx=ix;by=iy;bz=iz;if(tx<ty&&tx<tz){t=tx;tx+=sx;ix+=ax;}else if(ty<tz){t=ty;ty+=sy;iy+=ay;}else{t=tz;tz+=sz;iz+=az;}}return {false,0,0,0,0,0,0};}
static void capture(bool on){captured=on; if(on){SetCapture(window);while(ShowCursor(FALSE)>=0){} }else{ReleaseCapture();while(ShowCursor(TRUE)<0){}}}
static LRESULT CALLBACK proc(HWND h,UINT msg,WPARAM w,LPARAM l){switch(msg){case WM_CLOSE:running=false;return 0;case WM_DESTROY:running=false;return 0;case WM_SIZE:width=LOWORD(l);height=HIWORD(l);return 0;case WM_MOUSEWHEEL:wheel+=(GET_WHEEL_DELTA_WPARAM(w)>0?1:-1);return 0;case WM_LBUTTONDOWN:if(!captured){capture(true);previous_l=true;}return 0;case WM_KILLFOCUS:if(captured)capture(false);return 0;}return DefWindowProcW(h,msg,w,l);}
static bool key(int k){return (GetAsyncKeyState(k)&0x8000)!=0;}
static void input(double dt,double now){bool esc=key(VK_ESCAPE);if(esc&&!previous_escape&&captured)capture(false);previous_escape=esc;
    if(!captured)return;RECT r;GetClientRect(window,&r);POINT p={(r.right-r.left)/2,(r.bottom-r.top)/2};ClientToScreen(window,&p);POINT cur;GetCursorPos(&cur);yaw+=(cur.x-p.x)*.12f;pitch-=(cur.y-p.y)*.12f;pitch=std::max(-89.f,std::min(89.f,pitch));SetCursorPos(p.x,p.y);
    bool space=key(VK_SPACE);bool ground=collides(px,py-.06,pz);if(space&&!previous_space){if(now-last_space<.3){fly=!fly;vy=0;}else if(ground&&!fly)vy=8;last_space=now;}previous_space=space;
    for(int i=0;i<9;i++)if(key('1'+i))selected=i;if(wheel){selected=(selected-wheel%9+9)%9;wheel=0;}
    double forward=(key('W')?1:0)-(key('S')?1:0),side=(key('D')?1:0)-(key('A')?1:0),len=sqrt(forward*forward+side*side);if(len>0){forward/=len;side/=len;}
    double angle=yaw*.0174532925, speed=(key(VK_CONTROL)?8:5)*dt;double dx=(-sin(angle)*forward+cos(angle)*side)*speed,dz=(-cos(angle)*forward-sin(angle)*side)*speed;
    if(!collides(px+dx,py,pz))px+=dx;if(!collides(px,py,pz+dz))pz+=dz;
    if(fly)vy=((space?1:0)-(key(VK_SHIFT)?1:0))*7;else vy=std::max(-30.0,vy-22*dt);double dy=vy*dt;int steps=std::max(1,(int)ceil(fabs(dy)/.1));for(int i=0;i<steps;i++){if(!collides(px,py+dy/steps,pz))py+=dy/steps;else{vy=0;break;}}
    Hit h=ray();bool l=key(VK_LBUTTON),rr=key(VK_RBUTTON);if(h.yes&&l&&!previous_l)setblock(h.x,h.y,h.z,0);if(h.yes&&rr&&!previous_r){int x=h.prevx,y=h.prevy,z=h.prevz; if(!(px+.3>x&&px-.3<x+1&&py+1.8>y&&py<y+1&&pz+.3>z&&pz-.3<z+1))setblock(x,y,z,hotbar[selected]);}previous_l=l;previous_r=rr;
}
static void render(){if(width<1||height<1)return;glViewport(0,0,width,height);glClearColor(.52f,.72f,.91f,1);glClear(GL_COLOR_BUFFER_BIT|GL_DEPTH_BUFFER_BIT);glEnable(GL_DEPTH_TEST);glEnable(GL_FOG);GLfloat fog[]={.52f,.72f,.91f,1};glFogfv(GL_FOG_COLOR,fog);glFogi(GL_FOG_MODE,GL_LINEAR);glFogf(GL_FOG_START,32);glFogf(GL_FOG_END,58);
    glMatrixMode(GL_PROJECTION);glLoadIdentity();double znear=.1,top=znear*.7,right=top*width/height;glFrustum(-right,right,-top,top,znear,100);glMatrixMode(GL_MODELVIEW);glLoadIdentity();glRotatef(pitch,1,0,0);glRotatef(-yaw,0,1,0);glTranslated(-px,-py-1.62,-pz);
    glDisable(GL_BLEND);for(auto c:chunks){glPushMatrix();glTranslatef((float)(c->x*16),0,(float)(c->z*16));glCallList(c->opaque);glPopMatrix();}
    glEnable(GL_BLEND);glBlendFunc(GL_SRC_ALPHA,GL_ONE_MINUS_SRC_ALPHA);glDepthMask(GL_FALSE);for(auto c:chunks){glPushMatrix();glTranslatef((float)(c->x*16),0,(float)(c->z*16));glCallList(c->trans);glPopMatrix();}glDepthMask(GL_TRUE);glDisable(GL_BLEND);
    Hit h=ray();if(h.yes){glDisable(GL_FOG);glColor3f(0,0,0);float x=(float)h.x-.003f,y=(float)h.y-.003f,z=(float)h.z-.003f,s=1.006f;glBegin(GL_LINES);for(int a=0;a<2;a++)for(int b=0;b<2;b++){vertex(x,y+a*s,z+b*s);vertex(x+s,y+a*s,z+b*s);vertex(x+a*s,y,z+b*s);vertex(x+a*s,y+s,z+b*s);vertex(x+a*s,y+b*s,z);vertex(x+a*s,y+b*s,z+s);}glEnd();}
    glDisable(GL_FOG);glDisable(GL_DEPTH_TEST);glMatrixMode(GL_PROJECTION);glLoadIdentity();glOrtho(0,width,height,0,-1,1);glMatrixMode(GL_MODELVIEW);glLoadIdentity();glColor3f(1,1,1);glBegin(GL_LINES);vertex(width/2-7,height/2,0);vertex(width/2+7,height/2,0);vertex(width/2,height/2-7,0);vertex(width/2,height/2+7,0);glEnd();
    for(int i=0;i<9;i++){float x=width/2-162.f+i*36,y=height-45.f;glColor3f(i==selected?1:.15f,i==selected?1:.15f,i==selected?1:.15f);glBegin(GL_QUADS);vertex(x,y,0);vertex(x+34,y,0);vertex(x+34,y+34,0);vertex(x,y+34,0);glEnd();color(hotbar[i],3);glBegin(GL_QUADS);vertex(x+4,y+4,0);vertex(x+30,y+4,0);vertex(x+30,y+30,0);vertex(x+4,y+30,0);glEnd();}SwapBuffers(dc);
}
int WINAPI WinMain(HINSTANCE instance,HINSTANCE,LPSTR,int show){SetProcessDPIAware();WNDCLASSW wc={};wc.style=CS_OWNDC;wc.lpfnWndProc=proc;wc.hInstance=instance;wc.lpszClassName=L"RustcraftRedefinedWindow";wc.hCursor=LoadCursor(nullptr,IDC_ARROW);if(!RegisterClassW(&wc))return 1;window=CreateWindowW(wc.lpszClassName,L"Rustcraft Redefined",WS_OVERLAPPEDWINDOW,CW_USEDEFAULT,CW_USEDEFAULT,1024,720,nullptr,nullptr,instance,nullptr);if(!window)return 1;dc=GetDC(window);PIXELFORMATDESCRIPTOR pfd={};pfd.nSize=sizeof(pfd);pfd.nVersion=1;pfd.dwFlags=PFD_DRAW_TO_WINDOW|PFD_SUPPORT_OPENGL|PFD_DOUBLEBUFFER;pfd.iPixelType=PFD_TYPE_RGBA;pfd.cColorBits=32;pfd.cDepthBits=24;int pf=ChoosePixelFormat(dc,&pfd);if(!pf||!SetPixelFormat(dc,pf,&pfd))return 1;context=wglCreateContext(dc);if(!context||!wglMakeCurrent(dc,context))return 1;ShowWindow(window,show);init_save();LARGE_INTEGER freq,prev;QueryPerformanceFrequency(&freq);QueryPerformanceCounter(&prev);double now=0,title=0;int frames=0;
    while(running){MSG msg;while(PeekMessageW(&msg,nullptr,0,0,PM_REMOVE)){if(msg.message==WM_QUIT)running=false;TranslateMessage(&msg);DispatchMessageW(&msg);}LARGE_INTEGER t;QueryPerformanceCounter(&t);double dt=(double)(t.QuadPart-prev.QuadPart)/freq.QuadPart;prev=t;if(dt>.05)dt=.05;now+=dt;stream();input(dt,now);render();frames++;title+=dt;if(title>=1){wchar_t text[256];swprintf(text,256,L"Rustcraft Redefined | FPS %d | %.1f %.1f %.1f | seed 12345 | %s",(int)(frames/title),px,py,pz,fly?L"Fly":L"Walk");SetWindowTextW(window,text);title=0;frames=0;}Sleep(1);}
    if(captured)capture(false);save();for(auto c:chunks){glDeleteLists(c->opaque,1);glDeleteLists(c->trans,1);delete c;}wglMakeCurrent(nullptr,nullptr);wglDeleteContext(context);ReleaseDC(window,dc);DestroyWindow(window);return 0;
}
