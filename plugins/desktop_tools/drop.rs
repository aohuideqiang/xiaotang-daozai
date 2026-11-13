//! 拖放：把网页链接拖进岛体就能下载，文件仍然放进置物架。
//!
//! 岛仔原来的拖放由窗口库注册；插件接管后先撤销原来的目标，再注册自己的目标，
//! 因此两种拖放都走同一处，文件照旧交给 `App::on_drop`。
use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::{IDataObject, FORMATETC, DVASPECT_CONTENT, TYMED_HGLOBAL};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::*;
use windows::Win32::System::Ole::*;
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

static QUEUE: Mutex<Vec<Payload>> = Mutex::new(Vec::new());

/// 一次拖放的内容。
pub(crate) enum Payload {
    Url(String),
    Files(Vec<PathBuf>),
}

/// 取走这帧收到的拖放内容。
pub(crate) fn take() -> Vec<Payload> {
    std::mem::take(&mut *QUEUE.lock().unwrap_or_else(|error| error.into_inner()))
}

/// 只接受 http/https 链接，且不带账号密码。
fn valid_url(text: &str) -> Option<String> {
    let url = url::Url::parse(text.trim()).ok()?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() && url.username().is_empty() && url.password().is_none() {
        Some(url.into())
    } else {
        None
    }
}

unsafe fn read(data: &IDataObject) -> Option<Payload> {
    let url_format = RegisterClipboardFormatW(w!("UniformResourceLocatorW"));
    for format in [url_format as u16, CF_UNICODETEXT.0, CF_HDROP.0] {
        if format == 0 { continue; }
        let requested = FORMATETC { cfFormat: format, dwAspect: DVASPECT_CONTENT.0, lindex: -1, tymed: TYMED_HGLOBAL.0 as u32, ..Default::default() };
        let Ok(mut medium) = data.GetData(&requested) else { continue; };
        let payload = if format == CF_HDROP.0 {
            let drop = HDROP(medium.u.hGlobal.0);
            let count = DragQueryFileW(drop, u32::MAX, None);
            let mut paths = Vec::new();
            for index in 0..count.min(100) {
                let length = DragQueryFileW(drop, index, None) as usize;
                let mut buffer = vec![0u16; length + 1];
                DragQueryFileW(drop, index, Some(&mut buffer));
                paths.push(PathBuf::from(String::from_utf16_lossy(&buffer[..length])));
            }
            if paths.is_empty() { None } else { Some(Payload::Files(paths)) }
        } else {
            let handle = medium.u.hGlobal;
            let size = GlobalSize(handle) / 2;
            if size == 0 || size > 32768 {
                None
            } else {
                let pointer = GlobalLock(handle) as *const u16;
                if pointer.is_null() {
                    None
                } else {
                    let bytes = std::slice::from_raw_parts(pointer, size);
                    let length = bytes.iter().position(|value| *value == 0).unwrap_or(size);
                    let text = String::from_utf16_lossy(&bytes[..length]);
                    let _ = GlobalUnlock(handle);
                    valid_url(&text).map(Payload::Url)
                }
            }
        };
        ReleaseStgMedium(&mut medium);
        if payload.is_some() { return payload; }
    }
    None
}

#[implement(IDropTarget)]
struct Target {
    ctx: egui::Context,
    accepted: AtomicBool,
}

impl IDropTarget_Impl for Target_Impl {
    fn DragEnter(&self, data: Option<&IDataObject>, _keys: MODIFIERKEYS_FLAGS, _point: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        let accept = data.is_some_and(|data| unsafe { read(data).is_some() });
        self.accepted.store(accept, Ordering::Relaxed);

        unsafe { *effect = if accept { DROPEFFECT_COPY } else { DROPEFFECT_NONE }; }
        self.ctx.request_repaint();
        Ok(())
    }

    fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, _point: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        unsafe { *effect = if self.accepted.load(Ordering::Relaxed) { DROPEFFECT_COPY } else { DROPEFFECT_NONE }; }
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        self.ctx.request_repaint();
        Ok(())
    }

    fn Drop(&self, data: Option<&IDataObject>, _keys: MODIFIERKEYS_FLAGS, _point: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        let payload = data.and_then(|data| unsafe { read(data) });
        let accept = payload.is_some();
        if let Some(payload) = payload {
            if let Ok(mut queue) = QUEUE.lock() {
                if queue.len() < 100 { queue.push(payload); }
            }
        }
        unsafe { *effect = if accept { DROPEFFECT_COPY } else { DROPEFFECT_NONE }; }
        self.ctx.request_repaint();
        Ok(())
    }
}

/// 在岛体窗口上注册拖放目标；窗口库原有的目标会被替换。
pub(crate) fn install(hwnd: HWND, ctx: egui::Context) -> Result<()> {
    unsafe {
        let _ = RevokeDragDrop(hwnd);
        let target: IDropTarget = Target { ctx, accepted: AtomicBool::new(false) }.into();
        RegisterDragDrop(hwnd, &target)
    }
}
