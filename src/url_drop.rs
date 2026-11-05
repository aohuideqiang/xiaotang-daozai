//! OLE text/URL and file drops for the island; file staging remains supported.
use std::{path::PathBuf,sync::{Mutex,atomic::{AtomicBool,Ordering}}};
use windows::{core::*,Win32::{Foundation::*,System::{Com::*,Memory::*,Ole::*,SystemServices::MODIFIERKEYS_FLAGS,DataExchange::RegisterClipboardFormatW},UI::{Shell::{DragQueryFileW,HDROP}}}};
static QUEUE:Mutex<Vec<Payload>>=Mutex::new(Vec::new());
static HOVER:AtomicBool=AtomicBool::new(false);
pub enum Payload{Url(String),Files(Vec<PathBuf>)}
pub fn take()->Vec<Payload>{std::mem::take(&mut *QUEUE.lock().unwrap_or_else(|e|e.into_inner()))}
pub fn hovering()->bool{HOVER.load(Ordering::Relaxed)}
pub fn valid_url(text:&str)->Option<String>{let u=url::Url::parse(text.trim()).ok()?;if matches!(u.scheme(),"http"|"https")&&u.host_str().is_some()&&u.username().is_empty()&&u.password().is_none(){Some(u.into())}else{None}}
unsafe fn read(data:&IDataObject)->Option<Payload>{
 let urlformat=RegisterClipboardFormatW(w!("UniformResourceLocatorW"));
 for cf in [urlformat as u16,CF_UNICODETEXT.0,CF_HDROP.0]{
  if cf==0{continue;}let format=FORMATETC{cfFormat:cf,dwAspect:DVASPECT_CONTENT.0,lindex:-1,tymed:TYMED_HGLOBAL.0 as u32,..Default::default()};
  let Ok(mut medium)=data.GetData(&format)else{continue;};
  let result=if cf==CF_HDROP.0{
   let h=HDROP(medium.u.hGlobal.0);let count=DragQueryFileW(h,u32::MAX,None);let mut paths=vec![];
   for i in 0..count.min(100){let n=DragQueryFileW(h,i,None)as usize;let mut buf=vec![0;n+1];DragQueryFileW(h,i,Some(&mut buf));paths.push(PathBuf::from(String::from_utf16_lossy(&buf[..n])));}
   if paths.is_empty(){None}else{Some(Payload::Files(paths))}
  }else{
   let h=medium.u.hGlobal;let size=GlobalSize(h)/2;
   if size==0||size>32768{None}else{let p=GlobalLock(h)as *const u16;if p.is_null(){None}else{let b=std::slice::from_raw_parts(p,size);let n=b.iter().position(|c|*c==0).unwrap_or(size);let text=String::from_utf16_lossy(&b[..n]);let _=GlobalUnlock(h);valid_url(&text).map(Payload::Url)}}
  };
  ReleaseStgMedium(&mut medium);if result.is_some(){return result;}
 }None
}
#[implement(IDropTarget)]
struct Target{ctx:eframe::egui::Context,accepted:AtomicBool}
impl IDropTarget_Impl for Target_Impl{
 fn DragEnter(&self,data:Option<&IDataObject>,_:MODIFIERKEYS_FLAGS,_:&POINTL,effect:*mut DROPEFFECT)->Result<()>{
  let accept=data.is_some_and(|d|unsafe{read(d).is_some()});self.accepted.store(accept,Ordering::Relaxed);HOVER.store(accept,Ordering::Relaxed);unsafe{*effect=if accept{DROPEFFECT_COPY}else{DROPEFFECT_NONE};}self.ctx.request_repaint();Ok(())
 }
 fn DragOver(&self,_:MODIFIERKEYS_FLAGS,_:&POINTL,effect:*mut DROPEFFECT)->Result<()>{unsafe{*effect=if self.accepted.load(Ordering::Relaxed){DROPEFFECT_COPY}else{DROPEFFECT_NONE};}Ok(())}
 fn DragLeave(&self)->Result<()>{HOVER.store(false,Ordering::Relaxed);self.ctx.request_repaint();Ok(())}
 fn Drop(&self,data:Option<&IDataObject>,_:MODIFIERKEYS_FLAGS,_:&POINTL,effect:*mut DROPEFFECT)->Result<()>{
  HOVER.store(false,Ordering::Relaxed);let payload=data.and_then(|d|unsafe{read(d)});let accept=payload.is_some();if let Some(p)=payload{let mut queue=QUEUE.lock().unwrap_or_else(|e|e.into_inner());if queue.len()<100{queue.push(p);}}unsafe{*effect=if accept{DROPEFFECT_COPY}else{DROPEFFECT_NONE};}self.ctx.request_repaint();Ok(())
 }
}
pub fn install(hwnd:HWND,ctx:eframe::egui::Context)->Result<()>{unsafe{
 let _=RevokeDragDrop(hwnd);let target:IDropTarget=Target{ctx,accepted:AtomicBool::new(false)}.into();RegisterDragDrop(hwnd,&target)
}}
