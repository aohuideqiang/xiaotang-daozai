//! 系统跳转不可用时，定位播放器自己的进度控件并向该窗口发送消息。
use windows::{core::{Interface,VARIANT},Win32::{Foundation::*,System::{Com::*,Threading::*},UI::{Accessibility::*,WindowsAndMessaging::*},Graphics::Gdi::ScreenToClient}};
use windows::Win32::UI::HiDpi::{SetThreadDpiAwarenessContext,DPI_AWARENESS_CONTEXT,DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

struct DpiContext(DPI_AWARENESS_CONTEXT);
impl Drop for DpiContext{fn drop(&mut self){if !self.0.0.is_null(){unsafe{SetThreadDpiAwarenessContext(self.0);}}}}

fn seconds(text:&str)->Option<f64>{
    let parts=text.trim().split(':').map(str::parse::<u32>).collect::<Result<Vec<_>,_>>().ok()?;
    match parts.as_slice(){[m,s] if *s<60=>Some((m*60+s)as f64),[h,m,s] if *m<60&&*s<60=>Some((h*3600+m*60+s)as f64),_=>None}
}

fn point(rect:RECT,position:f64,duration:f64,inset:f64)->Option<POINT>{
    let width=(rect.right-rect.left)as f64;
    if !position.is_finite() || !duration.is_finite() || duration<=0.0 || position<0.0 || position>duration
        || width<=inset*2.0+10.0 || rect.bottom<=rect.top{return None;}
    Some(POINT{x:((rect.left as f64+inset+(width-2.0*inset)*position/duration).round()as i32).min(rect.right-1),y:rect.top+(rect.bottom-rect.top)/2})
}

unsafe fn matches_player(hwnd:HWND,source:&str)->bool{
    let mut pid=0;GetWindowThreadProcessId(hwnd,Some(&mut pid));
    let Ok(process)=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid)else{return false;};
    let mut name=[0u16;32768];let mut len=name.len()as u32;
    let result=QueryFullProcessImageNameW(process,PROCESS_NAME_WIN32,windows::core::PWSTR(name.as_mut_ptr()),&mut len);
    let _=CloseHandle(process);
    result.is_ok() && String::from_utf16_lossy(&name[..len as usize]).rsplit('\\').next().is_some_and(|name|name.eq_ignore_ascii_case(source))
}

unsafe fn window(source:&str)->Option<HWND>{
    struct Search<'a>{source:&'a str,found:Option<HWND>}
    unsafe extern "system" fn visit(hwnd:HWND,param:LPARAM)->BOOL{
        let search=&mut *(param.0 as *mut Search<'_>);
        let mut class=[0u16;80];let n=GetClassNameW(hwnd,&mut class);
        let class=String::from_utf16_lossy(&class[..n.max(0)as usize]);
        if class=="TXGuiFoundation" && IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd,GW_OWNER).is_err() && GetWindowTextLengthW(hwnd)>0 && matches_player(hwnd,search.source){search.found=Some(hwnd);return BOOL(0);}
        BOOL(1)
    }
    let mut search=Search{source,found:None};let _=EnumWindows(Some(visit),LPARAM(&mut search as *mut _ as isize));search.found
}

unsafe fn qq_bar(ui:&IUIAutomation,root:&IUIAutomationElement,duration:f64)->Option<IUIAutomationElement>{
    let condition=ui.CreatePropertyCondition(UIA_ControlTypePropertyId,&VARIANT::from(UIA_TextControlTypeId.0)).ok()?;
    let texts=root.FindAll(TreeScope_Descendants,&condition).ok()?;
    let walker=ui.ControlViewWalker().ok()?;let all=ui.CreateTrueCondition().ok()?;
    for i in 0..texts.Length().ok()? {
        let text=texts.GetElement(i).ok()?;
        if !seconds(&text.CurrentName().ok()?.to_string()).is_some_and(|v|(v-duration).abs()<1.5){continue;}
        let parent=walker.GetParentElement(&text).ok()?;
        let children=parent.FindAll(TreeScope_Children,&all).ok()?;
        if children.Length().ok()?!=3{continue;}
        let first=children.GetElement(0).ok()?;let last=children.GetElement(2).ok()?;
        if seconds(&first.CurrentName().ok()?.to_string()).is_none() || !ui.CompareElements(&last,&text).ok()?.as_bool(){continue;}
        let bar=children.GetElement(1).ok()?;
        let r=bar.CurrentBoundingRectangle().ok()?;
        let left=first.CurrentBoundingRectangle().ok()?;let right=last.CurrentBoundingRectangle().ok()?;
        if r.left>=left.right && r.right<=right.left && r.bottom-r.top<=60 && r.right-r.left>40{return Some(bar);}
    }
    None
}

pub(super) fn seek(source:&str,position:f64,duration:f64)->bool{
    if !source.eq_ignore_ascii_case("qqmusic.exe"){return false;}
    unsafe { (||->Option<()> {
        let _dpi=DpiContext(SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2));
        let hwnd=window(source)?;
        if !IsWindowEnabled(hwnd).as_bool(){return None;}
        let ui:IUIAutomation=CoCreateInstance(&CUIAutomation8,None,CLSCTX_INPROC_SERVER).ok()?;
        if let Ok(timed)=ui.cast::<IUIAutomation2>(){let _=timed.SetConnectionTimeout(800);let _=timed.SetTransactionTimeout(800);}
        let root=ui.ElementFromHandle(hwnd).ok()?;
        let bar=qq_bar(&ui,&root,duration)?;
        let target=hwnd;
        // 坐标来自无障碍控件，不依赖窗口位置；不移动鼠标，也不激活播放器。
        let mut p=point(bar.CurrentBoundingRectangle().ok()?,position,duration,5.0)?;
        if !ScreenToClient(target,&mut p).as_bool(){return None;}
        let mut client=RECT::default();GetClientRect(target,&mut client).ok()?;
        if p.x<0 || p.y<0 || (!IsIconic(hwnd).as_bool() && (p.x>=client.right || p.y>=client.bottom)){return None;}
        let lp=LPARAM(((p.y as u32&0xffff)<<16 | (p.x as u32&0xffff))as isize);
        let send=|msg,buttons|SendMessageTimeoutW(target,msg,WPARAM(buttons),lp,SMTO_ABORTIFHUNG|SMTO_ERRORONEXIT,500,None).0!=0;
        if !send(WM_MOUSEMOVE,0){return None;}
        let down=send(WM_LBUTTONDOWN,1);
        let up=send(WM_LBUTTONUP,0);
        (down&&up).then_some(())
    })().is_some() }
}

/// 网易云 3.x 的底部播放条由 CEF 渲染，不提供系统 seek 或 UIA 控件。
/// 主窗口最小化时渲染控件不存在，先把窗口临时透明还原（不激活、不闪现），
/// 点击后再恢复原状态并重新最小化；全程不移动鼠标、不抢占前台。
pub(super) fn netease_seek(position:f64,duration:f64)->bool {unsafe{
    use std::time::{Duration,Instant};
    use windows::core::{w,PCWSTR};
    let Ok(host)=FindWindowW(w!("OrpheusBrowserHost"),PCWSTR::null())else{return false;};
    if !matches_player(host,"cloudmusic.exe") || !IsWindowEnabled(host).as_bool(){return false;}
    let _dpi=DpiContext(SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2));
    let scale=windows::Win32::UI::HiDpi::GetDpiForWindow(host)as f64/96.0;
    struct Search(Option<HWND>);
    unsafe extern "system" fn visit(hwnd:HWND,param:LPARAM)->BOOL {
        let mut name=[0u16;80];let n=GetClassNameW(hwnd,&mut name);
        let class=String::from_utf16_lossy(&name[..n.max(0)as usize]);
        // 输入控件在旧版叫 Chrome_RenderWidgetHostHWND，新版是 Chrome_WidgetWin_0。
        if class=="Chrome_RenderWidgetHostHWND" || class=="Chrome_WidgetWin_0" {
            (*(param.0 as *mut Search)).0=Some(hwnd);return BOOL(0);
        }
        BOOL(1)
    }
    unsafe fn widget(host:HWND,scale:f64,visit:unsafe extern "system" fn(HWND,LPARAM)->BOOL)->Option<HWND>{
        let mut search=Search(None);
        let _=EnumChildWindows(host,Some(visit),LPARAM(&mut search as *mut _ as isize));
        search.0.filter(|hwnd|{
            let mut rect=RECT::default();
            GetClientRect(*hwnd,&mut rect).is_ok() && scale>0.0
                && rect.right as f64/scale>=800.0 && rect.bottom as f64/scale>=550.0
        })
    }
    // 最小化时没有可点击控件：透明还原后等待控件出现；drop 时恢复透明度并最小化。
    struct Restore(HWND,bool,isize,bool);
    impl Drop for Restore {
        fn drop(&mut self){
            if !self.1{return;}
            unsafe{
                let _=SetLayeredWindowAttributes(self.0,COLORREF::default(),255,LWA_ALPHA);
                if !self.3{let _=SetWindowLongPtrW(self.0,GWL_EXSTYLE,self.2);}
                let _=ShowWindowAsync(self.0,SW_MINIMIZE);
            }
        }
    }
    let mut restore=Restore(host,false,0,true);
    let Some(target)=widget(host,scale,visit).or_else(||{
        if !IsIconic(host).as_bool(){return None;}
        let exstyle=GetWindowLongPtrW(host,GWL_EXSTYLE);
        let layered=exstyle & WS_EX_LAYERED.0 as isize!=0;
        if !layered{let _=SetWindowLongPtrW(host,GWL_EXSTYLE,exstyle|WS_EX_LAYERED.0 as isize);}
        let _=SetLayeredWindowAttributes(host,COLORREF::default(),0,LWA_ALPHA);
        let _=ShowWindowAsync(host,SW_SHOWNOACTIVATE);
        restore.1=true;restore.2=exstyle;restore.3=layered;
        let deadline=Instant::now()+Duration::from_millis(1200);
        loop{
            if let Some(hwnd)=widget(host,scale,visit){return Some(hwnd);}
            if Instant::now()>=deadline{return None;}
            std::thread::sleep(Duration::from_millis(50));
        }
    }) else{return false;};
    let mut rect=RECT::default();if GetClientRect(target,&mut rect).is_err(){return false;}
    let Some(mut p)=point(rect,position,duration,1.0)else{return false;};
    p.y=rect.bottom-(82.0*scale).round()as i32;
    let lp=LPARAM(((p.y as u32&0xffff)<<16 | (p.x as u32&0xffff))as isize);
    let send=|msg,buttons|SendMessageTimeoutW(target,msg,WPARAM(buttons),lp,SMTO_ABORTIFHUNG|SMTO_ERRORONEXIT,500,None).0!=0;
    if !send(WM_MOUSEMOVE,0){return false;}
    let down=send(WM_LBUTTONDOWN,1);let up=send(WM_LBUTTONUP,0);down&&up
}}

#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn progress_coordinates_follow_control_bounds_and_reject_invalid_input(){
        for left in [-31000,100,2300]{let r=RECT{left,top:500,right:left+260,bottom:524};
            assert_eq!(point(r,0.0,200.0,5.0).unwrap().x,left+5);
            assert_eq!(point(r,100.0,200.0,5.0).unwrap().x,left+130);
            assert_eq!(point(r,200.0,200.0,5.0).unwrap().x,left+255);
            assert_eq!(point(r,200.0,200.0,0.0).unwrap().x,r.right-1);
            for p in [f64::NAN,-1.0,201.0]{assert!(point(r,p,200.0,5.0).is_none());}
            assert!(point(r,0.0,0.0,5.0).is_none());
        }
        assert_eq!(seconds(" 04:41"),Some(281.0));assert_eq!(seconds("01:02:03"),Some(3723.0));
        for text in ["播放","04:99","", "1:2:3:4"]{assert!(seconds(text).is_none());}
    }
}
