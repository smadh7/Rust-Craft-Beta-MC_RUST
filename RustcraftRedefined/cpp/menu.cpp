#define WIN32_LEAN_AND_MEAN
#include "menu.h"
#include "world_names.h"
#include "world_format.h"
#include <stdio.h>
#include <string.h>

namespace {
const wchar_t* graphics_names[]={L"Potato / Basic",L"Normal",L"High",L"Very High / Peak"};
const wchar_t* skin_names[]={L"Explorer - warm",L"Explorer - deep",L"Explorer - light",L"Explorer - cool"};
enum { LIST=100, WORLD_NAME, SEED, GRAPHICS, SKIN, CREATE_WORLD, PLAY_WORLD, QUICK_PLAY };
struct Menu {
    HWND window=nullptr,list=nullptr,name=nullptr,seed=nullptr,graphics=nullptr,skin=nullptr;
    HINSTANCE instance=nullptr; int dpi=96; bool done=false;
    std::wstring worlds; MenuResult result;
};
int scale(Menu* m,int n){return MulDiv(n,m->dpi,96);}
bool ensure_dir(const std::wstring& path){
    if(CreateDirectoryW(path.c_str(),nullptr))return true;
    if(GetLastError()!=ERROR_ALREADY_EXISTS)return false;
    DWORD attr=GetFileAttributesW(path.c_str());return attr!=INVALID_FILE_ATTRIBUTES&&(attr&FILE_ATTRIBUTE_DIRECTORY);
}
std::wstring data_directory(){
    const wchar_t* envs[]={L"APPDATA",L"USERPROFILE"};
    for(const wchar_t* env:envs){wchar_t base[32768];DWORD n=GetEnvironmentVariableW(env,base,32768);if(!n||n>=32768)continue;
        std::wstring path=std::wstring(base)+L"\\Rustcraft Redefined";if(ensure_dir(path))return path;}
    return L"";
}
void load_preferences(MenuResult& state){
    if(state.data_dir.empty())return;std::wstring path=state.data_dir+L"\\settings.bin";FILE*f=_wfopen(path.c_str(),L"rb");if(!f)return;
    char magic[8];unsigned char values[2];
    if(fread(magic,1,8,f)==8&&memcmp(magic,"RCRCFG01",8)==0&&fread(values,1,2,f)==2&&values[0]<4&&values[1]<4){state.graphics=values[0];state.skin=values[1];}
    fclose(f);
}
bool read_world_header(const std::wstring& path,uint64_t& seed){
    HANDLE f=CreateFileW(path.c_str(),GENERIC_READ,FILE_SHARE_READ,nullptr,OPEN_EXISTING,FILE_ATTRIBUTE_NORMAL,nullptr);if(f==INVALID_HANDLE_VALUE)return false;
    unsigned char header[20];DWORD read=0;LARGE_INTEGER length;bool ok=ReadFile(f,header,sizeof header,&read,nullptr)&&read==sizeof header&&GetFileSizeEx(f,&length);
    if(ok){uint32_t count;ok=decode_world_header(header,length.QuadPart,seed,count);}
    CloseHandle(f);return ok;
}
bool create_empty_world(const std::wstring& path,uint64_t seed){
    HANDLE f=CreateFileW(path.c_str(),GENERIC_WRITE,0,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr);if(f==INVALID_HANDLE_VALUE)return false;
    unsigned char header[20];encode_world_header(header,seed,0);
    DWORD wrote=0;bool ok=WriteFile(f,header,sizeof header,&wrote,nullptr)&&wrote==sizeof header;
    if(!CloseHandle(f))ok=false;if(!ok)DeleteFileW(path.c_str());return ok;
}
void populate_list(Menu* m){
    SendMessageW(m->list,LB_RESETCONTENT,0,0);if(m->worlds.empty())return;
    WIN32_FIND_DATAW data;HANDLE search=FindFirstFileW((m->worlds+L"\\*.world").c_str(),&data);if(search==INVALID_HANDLE_VALUE)return;
    do {if(data.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY)continue;std::wstring name=data.cFileName;
        if(name.size()<7)continue;name.resize(name.size()-6);if(!valid_world_name(name))continue;
        SendMessageW(m->list,LB_ADDSTRING,0,(LPARAM)name.c_str());
    }while(FindNextFileW(search,&data));FindClose(search);
    if(SendMessageW(m->list,LB_GETCOUNT,0,0)>0)SendMessageW(m->list,LB_SETCURSEL,0,0);
}
HWND control(Menu*m,const wchar_t* cls,const wchar_t* text,DWORD style,int x,int y,int w,int h,int id=0,DWORD ex=0){
    HWND c=CreateWindowExW(ex,cls,text,WS_CHILD|WS_VISIBLE|style,scale(m,x),scale(m,y),scale(m,w),scale(m,h),m->window,(HMENU)(INT_PTR)id,m->instance,nullptr);
    if(c)SendMessageW(c,WM_SETFONT,(WPARAM)GetStockObject(DEFAULT_GUI_FONT),TRUE);return c;
}
void apply_selection(Menu* m){
    int g=(int)SendMessageW(m->graphics,CB_GETCURSEL,0,0),s=(int)SendMessageW(m->skin,CB_GETCURSEL,0,0);
    if(g>=0&&g<4)m->result.graphics=g;if(s>=0&&s<4)m->result.skin=s;
    save_preferences(m->result);
}
bool input_seed(Menu*m,uint64_t& seed){wchar_t text[32];GetWindowTextW(m->seed,text,32);if(parse_world_seed(text,seed))return true;
    MessageBoxW(m->window,L"Enter a whole-number seed from 0 to 18446744073709551615.",L"Invalid seed",MB_OK|MB_ICONWARNING);return false;}
void launch(Menu*m,const std::wstring& path,const std::wstring& name,uint64_t seed){
    m->result.save_path=path;m->result.world_name=name;m->result.seed=seed;apply_selection(m);m->result.start=true;DestroyWindow(m->window);
}
LRESULT CALLBACK menu_proc(HWND h,UINT msg,WPARAM w,LPARAM l){
    Menu* m=(Menu*)GetWindowLongPtrW(h,GWLP_USERDATA);
    if(msg==WM_NCCREATE){m=(Menu*)((CREATESTRUCTW*)l)->lpCreateParams;SetWindowLongPtrW(h,GWLP_USERDATA,(LONG_PTR)m);m->window=h;}
    if(!m)return DefWindowProcW(h,msg,w,l);
    switch(msg){
    case WM_CREATE:{
        control(m,L"STATIC",L"RUSTCRAFT REDEFINED",SS_LEFT,24,18,540,25);
        control(m,L"STATIC",L"Offline worlds  |  Choose your look, then explore.",SS_LEFT,24,45,540,22);
        control(m,L"STATIC",L"Your worlds",SS_LEFT,24,82,230,20);
        m->list=control(m,L"LISTBOX",L"",LBS_NOTIFY|LBS_NOINTEGRALHEIGHT|LBS_SORT|WS_VSCROLL|WS_TABSTOP,24,105,260,170,LIST,WS_EX_CLIENTEDGE);
        control(m,L"BUTTON",L"Play selected world",BS_PUSHBUTTON|WS_TABSTOP,24,286,260,32,PLAY_WORLD);
        control(m,L"STATIC",L"New world name",SS_LEFT,315,82,240,20);
        m->name=control(m,L"EDIT",L"My World",ES_AUTOHSCROLL|WS_TABSTOP,315,105,245,26,WORLD_NAME,WS_EX_CLIENTEDGE);SendMessageW(m->name,EM_SETLIMITTEXT,40,0);
        control(m,L"STATIC",L"Seed (whole number)",SS_LEFT,315,142,240,20);
        m->seed=control(m,L"EDIT",L"12345",ES_AUTOHSCROLL|WS_TABSTOP,315,165,245,26,SEED,WS_EX_CLIENTEDGE);SendMessageW(m->seed,EM_SETLIMITTEXT,21,0);
        control(m,L"BUTTON",L"Create world & play",BS_PUSHBUTTON|WS_TABSTOP,315,203,245,32,CREATE_WORLD);
        control(m,L"STATIC",L"Graphics",SS_LEFT,24,345,140,20);
        m->graphics=control(m,L"COMBOBOX",L"",CBS_DROPDOWNLIST|WS_VSCROLL|WS_TABSTOP,24,368,260,150,GRAPHICS);
        for(const auto* text:graphics_names)SendMessageW(m->graphics,CB_ADDSTRING,0,(LPARAM)text);SendMessageW(m->graphics,CB_SETCURSEL,m->result.graphics,0);
        control(m,L"STATIC",L"Skin (first-person hand)",SS_LEFT,315,345,245,20);
        m->skin=control(m,L"COMBOBOX",L"",CBS_DROPDOWNLIST|WS_VSCROLL|WS_TABSTOP,315,368,245,150,SKIN);
        for(const auto* text:skin_names)SendMessageW(m->skin,CB_ADDSTRING,0,(LPARAM)text);SendMessageW(m->skin,CB_SETCURSEL,m->result.skin,0);
        control(m,L"BUTTON",L"Quick play (not saved)",BS_PUSHBUTTON|WS_TABSTOP,24,418,260,32,QUICK_PLAY);
        control(m,L"STATIC",m->worlds.empty()?L"Storage unavailable: only unsaved quick play is available.":L"World edits save locally. Quick play does not save.",SS_LEFT,24,464,540,35);
        if(m->worlds.empty()){EnableWindow(GetDlgItem(h,CREATE_WORLD),FALSE);EnableWindow(GetDlgItem(h,PLAY_WORLD),FALSE);}
        populate_list(m);return 0;}
    case WM_COMMAND:{int id=LOWORD(w),notification=HIWORD(w);
        if(id==CREATE_WORLD&&notification==BN_CLICKED){wchar_t name[42];GetWindowTextW(m->name,name,42);std::wstring n=name;uint64_t seed;
            if(!valid_world_name(n)){MessageBoxW(h,L"Use 1–40 letters, numbers, spaces, hyphens or underscores. Avoid leading or trailing spaces and reserved device names.",L"Invalid world name",MB_OK|MB_ICONWARNING);return 0;}
            if(!input_seed(m,seed))return 0;std::wstring path=m->worlds+L"\\"+n+L".world";
            if(!create_empty_world(path,seed)){MessageBoxW(h,L"Could not create this world. The name may already exist, or the folder may not be writable.",L"World not created",MB_OK|MB_ICONERROR);return 0;}
            launch(m,path,n,seed);return 0;}
        if((id==PLAY_WORLD&&notification==BN_CLICKED)||(id==LIST&&notification==LBN_DBLCLK)){
            LRESULT index=SendMessageW(m->list,LB_GETCURSEL,0,0);if(index==LB_ERR){MessageBoxW(h,L"Select a world first, or create one.",L"Choose a world",MB_OK|MB_ICONINFORMATION);return 0;}
            wchar_t name[41];if(SendMessageW(m->list,LB_GETTEXT,index,(LPARAM)name)==LB_ERR)return 0;
            std::wstring path=m->worlds+L"\\"+name+L".world";uint64_t seed;
            if(!read_world_header(path,seed)){MessageBoxW(h,L"This world could not be read or may be damaged. It has not been changed.",L"Unable to open world",MB_OK|MB_ICONERROR);return 0;}
            launch(m,path,name,seed);return 0;}
        if(id==QUICK_PLAY&&notification==BN_CLICKED){uint64_t seed;if(input_seed(m,seed))launch(m,L"",L"Quick play (unsaved)",seed);return 0;}
        return 0;}
    case WM_CLOSE:DestroyWindow(h);return 0;
    case WM_DESTROY:m->done=true;return 0;
    }
    return DefWindowProcW(h,msg,w,l);
}
}
void save_preferences(const MenuResult& state){
    if(state.data_dir.empty())return;std::wstring path=state.data_dir+L"\\settings.bin",tmp=path+L".tmp";
    FILE*f=_wfopen(tmp.c_str(),L"wb");if(!f)return;unsigned char values[2]={(unsigned char)state.graphics,(unsigned char)state.skin};
    bool ok=fwrite("RCRCFG01",1,8,f)==8&&fwrite(values,1,2,f)==2;if(fclose(f)!=0)ok=false;
    if(ok)MoveFileExW(tmp.c_str(),path.c_str(),MOVEFILE_REPLACE_EXISTING);else DeleteFileW(tmp.c_str());
}
MenuResult open_start_menu(HINSTANCE instance){
    Menu m;m.instance=instance;m.result.data_dir=data_directory();
    if(!m.result.data_dir.empty()){
        std::wstring worlds=m.result.data_dir+L"\\Worlds";if(ensure_dir(worlds))m.worlds=worlds;
        if(!m.worlds.empty()){
            std::wstring old=m.result.data_dir+L"\\world.bin",legacy=m.worlds+L"\\Legacy World.world";
            DWORD a=GetFileAttributesW(old.c_str()),b=GetFileAttributesW(legacy.c_str());
            if(a!=INVALID_FILE_ATTRIBUTES&&!(a&FILE_ATTRIBUTE_DIRECTORY)&&b==INVALID_FILE_ATTRIBUTES){
                uint64_t seed;
                if(!read_world_header(old,seed)||!CopyFileW(old.c_str(),legacy.c_str(),TRUE))
                    MessageBoxW(nullptr,L"The previous save could not be copied to the world list. It may be damaged or the destination may not be writable. Your original world.bin was not changed.",L"Legacy world",MB_OK|MB_ICONWARNING);
            }
        }
    }
    load_preferences(m.result);HDC screen=GetDC(nullptr);if(screen){m.dpi=GetDeviceCaps(screen,LOGPIXELSX);ReleaseDC(nullptr,screen);}
    WNDCLASSW wc={};wc.lpfnWndProc=menu_proc;wc.hInstance=instance;wc.lpszClassName=L"RustcraftStartMenu";wc.hCursor=LoadCursor(nullptr,IDC_ARROW);wc.hbrBackground=(HBRUSH)(COLOR_BTNFACE+1);
    if(!RegisterClassW(&wc))return m.result;
    RECT r={0,0,scale(&m,584),scale(&m,510)};DWORD style=WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|WS_MINIMIZEBOX;AdjustWindowRect(&r,style,FALSE);
    HWND h=CreateWindowW(wc.lpszClassName,L"Rustcraft Redefined - Choose a world",style,CW_USEDEFAULT,CW_USEDEFAULT,r.right-r.left,r.bottom-r.top,nullptr,nullptr,instance,&m);
    if(!h)return m.result;ShowWindow(h,SW_SHOW);UpdateWindow(h);MSG message;
    while(!m.done){BOOL status=GetMessageW(&message,nullptr,0,0);if(status<=0)break;if(!IsDialogMessageW(h,&message)){TranslateMessage(&message);DispatchMessageW(&message);}}
    return m.result;
}
