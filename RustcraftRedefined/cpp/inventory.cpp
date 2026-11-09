#define WIN32_LEAN_AND_MEAN
#include "inventory.h"
#include <stdio.h>
namespace {
struct Entry { uint8_t id; const wchar_t* name; };
const Entry entries[]={{1,L"Stone"},{2,L"Dirt"},{3,L"Grass"},{4,L"Sand"},{5,L"Water"},{6,L"Log"},{7,L"Leaves"},{8,L"Planks"},{10,L"Glass"},{11,L"Snow"},{12,L"Gravel"},{13,L"Brick"}};
struct Dialog { HWND window=nullptr,list=nullptr; HINSTANCE instance=nullptr; const PlayerState* player=nullptr; int slot=0;uint8_t current=0,result=0;bool done=false;int dpi=96; };
int scale(Dialog*d,int n){return MulDiv(n,d->dpi,96);}
HWND control(Dialog*d,const wchar_t* cls,const wchar_t* text,DWORD style,int x,int y,int w,int h,int id,DWORD ex=0){
    HWND c=CreateWindowExW(ex,cls,text,WS_CHILD|WS_VISIBLE|style,scale(d,x),scale(d,y),scale(d,w),scale(d,h),d->window,(HMENU)(INT_PTR)id,d->instance,nullptr);
    if(c)SendMessageW(c,WM_SETFONT,(WPARAM)GetStockObject(DEFAULT_GUI_FONT),TRUE);return c;
}
void accept(Dialog*d){
    LRESULT i=SendMessageW(d->list,LB_GETCURSEL,0,0);if(i==LB_ERR){MessageBoxW(d->window,L"Choose a block first.",L"Inventory",MB_OK|MB_ICONINFORMATION);return;}
    LRESULT id=SendMessageW(d->list,LB_GETITEMDATA,(WPARAM)i,0);if(id<=0||id>13)return;
    d->result=(uint8_t)id;DestroyWindow(d->window);
}
LRESULT CALLBACK proc(HWND h,UINT msg,WPARAM w,LPARAM l){
    Dialog*d=(Dialog*)GetWindowLongPtrW(h,GWLP_USERDATA);
    if(msg==WM_NCCREATE){d=(Dialog*)((CREATESTRUCTW*)l)->lpCreateParams;d->window=h;SetWindowLongPtrW(h,GWLP_USERDATA,(LONG_PTR)d);}
    if(!d)return DefWindowProcW(h,msg,w,l);
    switch(msg){
    case WM_CREATE:{wchar_t title[80];swprintf(title,80,L"Choose a block for hotbar slot %d",d->slot+1);
        control(d,L"STATIC",title,0,18,16,340,20,0);
        d->list=control(d,L"LISTBOX",L"",LBS_NOTIFY|LBS_NOINTEGRALHEIGHT|WS_VSCROLL|WS_TABSTOP,18,42,340,220,100,WS_EX_CLIENTEDGE);
        for(const auto&e:entries){wchar_t label[128];if(limited(*d->player))swprintf(label,128,L"%ls  -  %u available",e.name,(unsigned)d->player->blocks[e.id]);else swprintf(label,128,L"%ls  -  unlimited",e.name);
            LRESULT i=SendMessageW(d->list,LB_ADDSTRING,0,(LPARAM)label);if(i!=LB_ERR&&i!=LB_ERRSPACE){SendMessageW(d->list,LB_SETITEMDATA,(WPARAM)i,e.id);if(e.id==d->current)SendMessageW(d->list,LB_SETCURSEL,(WPARAM)i,0);}}
        control(d,L"BUTTON",L"Assign to slot",BS_DEFPUSHBUTTON|WS_TABSTOP,18,277,163,30,101);
        control(d,L"BUTTON",L"Cancel",BS_PUSHBUTTON|WS_TABSTOP,195,277,163,30,102);
        control(d,L"STATIC",limited(*d->player)?L"Empty selections cannot be placed until collected.":L"Creative blocks are unlimited. Water does not flow yet.",0,18,320,345,36,0);
        SetFocus(d->list);return 0;}
    case WM_COMMAND:if((LOWORD(w)==101&&HIWORD(w)==BN_CLICKED)||(LOWORD(w)==100&&HIWORD(w)==LBN_DBLCLK)){accept(d);return 0;}if(LOWORD(w)==102&&HIWORD(w)==BN_CLICKED){DestroyWindow(h);return 0;}break;
    case WM_CLOSE:DestroyWindow(h);return 0;
    case WM_DESTROY:d->done=true;return 0;
    }return DefWindowProcW(h,msg,w,l);
}
}
uint8_t open_block_inventory(HWND parent,HINSTANCE instance,const PlayerState& player,int slot,uint8_t current){
    Dialog d;d.instance=instance;d.player=&player;d.slot=slot;d.current=current;
    HDC screen=GetDC(nullptr);if(screen){d.dpi=GetDeviceCaps(screen,LOGPIXELSX);ReleaseDC(nullptr,screen);}
    WNDCLASSW wc={};wc.lpfnWndProc=proc;wc.hInstance=instance;wc.lpszClassName=L"RustcraftInventory";wc.hCursor=LoadCursor(nullptr,IDC_ARROW);wc.hbrBackground=(HBRUSH)(COLOR_BTNFACE+1);
    if(!RegisterClassW(&wc)&&GetLastError()!=ERROR_CLASS_ALREADY_EXISTS)return 0;
    DWORD style=WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU;RECT r={0,0,scale(&d,376),scale(&d,366)};AdjustWindowRect(&r,style,FALSE);
    HWND h=CreateWindowExW(WS_EX_DLGMODALFRAME,wc.lpszClassName,L"Rustcraft Redefined - Inventory",style,CW_USEDEFAULT,CW_USEDEFAULT,r.right-r.left,r.bottom-r.top,parent,nullptr,instance,&d);
    if(!h)return 0;EnableWindow(parent,FALSE);ShowWindow(h,SW_SHOW);UpdateWindow(h);MSG msg;
    while(!d.done){BOOL status=GetMessageW(&msg,nullptr,0,0);if(status<=0){if(status==0)PostQuitMessage((int)msg.wParam);break;}if(!IsDialogMessageW(h,&msg)){TranslateMessage(&msg);DispatchMessageW(&msg);}}
    if(IsWindow(h))DestroyWindow(h);EnableWindow(parent,TRUE);SetForegroundWindow(parent);return d.result;
}
