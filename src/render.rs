//! Event-driven premultiplied-alpha drawing. All native drawing resources are scoped.
#![allow(unsafe_op_in_unsafe_fn)]
use std::{mem::{size_of, zeroed}, ptr::{null, null_mut}};
use windows_sys::Win32::{Foundation::*, Graphics::{Gdi::*, GdiPlus::*}, UI::WindowsAndMessaging::*};
use crate::model::Document;

// Keep all theme colors together so visual changes don't affect interaction code.
struct Palette {
    background: u32,
    text: u32,
    muted: u32,
    accent: u32,
    divider: u32,
    hover: u32,
    input: u32,
    checkbox_fill: u32,
    checkbox_border: u32,
    checkmark: u32,
}
impl Palette {
    fn new(light: bool) -> Self {
        if light {
            Self {
                background: 0x00f4f7f8, text: 0xff24343d, muted: 0xff52656c,
                accent: 0xff267956, divider: 0x20394d57, hover: 0x143d7965,
                input: 0x1643545f, checkbox_fill: 0xd9ffffff,
                checkbox_border: 0xff73878e, checkmark: 0xfff7fffb,
            }
        } else {
            Self {
                background: 0x001c2832, text: 0xfff3f6f8, muted: 0xffc2ccd3,
                accent: 0xffa8e6cb, divider: 0x1fffffff, hover: 0x14ffffff,
                input: 0x12ffffff, checkbox_fill: 0x16ffffff,
                checkbox_border: 0xffe9f0f4, checkmark: 0xff1c3c30,
            }
        }
    }
}
pub fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }
pub struct GdiSession(usize);
impl GdiSession {
    pub fn new() -> Result<Self, String> { unsafe {
        let mut token = 0;
        let input = GdiplusStartupInput { GdiplusVersion: 1, DebugEventCallback: 0, SuppressBackgroundThread: 0, SuppressExternalCodecs: 1 };
        if GdiplusStartup(&mut token, &input, null_mut()) != 0 { return Err("无法初始化系统绘图库".into()); }
        Result::Ok(Self(token))
    } }
}
impl Drop for GdiSession { fn drop(&mut self) { unsafe { GdiplusShutdown(self.0); } } }

struct Canvas { dc: HDC, bitmap: HBITMAP, old: HGDIOBJ, image: *mut GpBitmap,
    g: *mut GpGraphics, family: *mut GpFontFamily, width: i32, height: i32 }
impl Canvas {
    unsafe fn new(width: i32, height: i32, scale: f32) -> Result<Self, String> {
        let dc = CreateCompatibleDC(null_mut());
        let mut c = Self { dc, bitmap: null_mut(), old: null_mut(), image: null_mut(), g: null_mut(), family: null_mut(), width, height };
        if dc.is_null() { return Err("无法创建绘图上下文".into()); }
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width; info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1; info.bmiHeader.biBitCount = 32;
        let mut bits = null_mut();
        c.bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if c.bitmap.is_null() { return Err("无法创建透明位图".into()); }
        c.old = SelectObject(dc, c.bitmap);
        // PixelFormat32bppPARGB: the memory format required by UpdateLayeredWindow.
        if GdipCreateBitmapFromScan0(width,height,width*4,0x000e200b,bits.cast(),&mut c.image) != 0
            || GdipGetImageGraphicsContext(c.image.cast(),&mut c.g) != 0 { return Err("无法初始化透明画布".into()); }
        GdipGraphicsClear(c.g,0);
        GdipSetSmoothingMode(c.g,SmoothingModeAntiAlias);
        GdipSetTextRenderingHint(c.g,TextRenderingHintAntiAliasGridFit);
        GdipScaleWorldTransform(c.g,scale,scale,MatrixOrderPrepend);
        if GdipCreateFontFamilyFromName(wide("Microsoft YaHei UI").as_ptr(),null_mut(),&mut c.family) != 0 {
            if GdipGetGenericFontFamilySansSerif(&mut c.family) != 0 { return Err("无法加载系统字体".into()); }
        }
        Result::Ok(c)
    }
    unsafe fn round(&self,x:f32,y:f32,w:f32,h:f32,r:f32,color:u32) {
        let mut p=null_mut(); let mut b=null_mut();
        GdipCreatePath(FillModeAlternate,&mut p); GdipCreateSolidFill(color,&mut b);
        let d = r*2.;
        GdipAddPathArc(p,x,y,d,d,180.,90.); GdipAddPathArc(p,x+w-d,y,d,d,270.,90.);
        GdipAddPathArc(p,x+w-d,y+h-d,d,d,0.,90.); GdipAddPathArc(p,x,y+h-d,d,d,90.,90.);
        GdipClosePathFigure(p); GdipFillPath(self.g,b.cast(),p); GdipDeletePath(p); GdipDeleteBrush(b.cast());
    }
    unsafe fn line(&self,x:f32,y:f32,x2:f32,y2:f32,color:u32,width:f32) {
        let mut p=null_mut(); GdipCreatePen1(color,width,UnitPixel,&mut p);
        GdipDrawLine(self.g,p,x,y,x2,y2); GdipDeletePen(p);
    }
    unsafe fn outline(&self,x:f32,y:f32,w:f32,h:f32,r:f32,color:u32,width:f32) {
        let mut path=null_mut(); let mut pen=null_mut();
        GdipCreatePath(FillModeAlternate,&mut path);
        let d=r*2.;
        GdipAddPathArc(path,x,y,d,d,180.,90.); GdipAddPathArc(path,x+w-d,y,d,d,270.,90.);
        GdipAddPathArc(path,x+w-d,y+h-d,d,d,0.,90.); GdipAddPathArc(path,x,y+h-d,d,d,90.,90.);
        GdipClosePathFigure(path); GdipCreatePen1(color,width,UnitPixel,&mut pen);
        GdipDrawPath(self.g,pen,path); GdipDeletePen(pen); GdipDeletePath(path);
    }
    unsafe fn text(&self,s:&str,x:f32,y:f32,w:f32,h:f32,size:f32,color:u32,bold:bool) {
        let mut f=null_mut(); let mut b=null_mut(); let mut fmt=null_mut();
        GdipCreateFont(self.family,size,if bold {FontStyleBold} else {FontStyleRegular},UnitPixel,&mut f);
        GdipCreateSolidFill(color,&mut b); GdipCreateStringFormat(0,0,&mut fmt);
        GdipSetStringFormatTrimming(fmt,StringTrimmingEllipsisCharacter);
        let s=wide(s); let rect=RectF{X:x,Y:y,Width:w,Height:h};
        GdipDrawString(self.g,s.as_ptr(),(s.len()-1) as i32,f,&rect,fmt,b.cast());
        GdipDeleteStringFormat(fmt); GdipDeleteBrush(b.cast()); GdipDeleteFont(f);
    }
    unsafe fn measure(&self,s:&str,w:f32)->f32 {
        let mut f=null_mut(); GdipCreateFont(self.family,14.,FontStyleRegular,UnitPixel,&mut f);
        let s=wide(s); let rect=RectF{X:0.,Y:0.,Width:w,Height:1000.}; let mut bounds:RectF=zeroed();
        GdipMeasureString(self.g,s.as_ptr(),(s.len()-1) as i32,f,&rect,null(),&mut bounds,null_mut(),null_mut());
        GdipDeleteFont(f); bounds.Height.clamp(22.,88.)
    }
    unsafe fn present(&self, hwnd:HWND)->Result<(),String> {
        GdipFlush(self.g,FlushIntentionSync);
        let size=SIZE{cx:self.width,cy:self.height}; let origin=POINT{x:0,y:0};
        let blend=BLENDFUNCTION{BlendOp:AC_SRC_OVER as u8,BlendFlags:0,SourceConstantAlpha:255,AlphaFormat:AC_SRC_ALPHA as u8};
        if UpdateLayeredWindow(hwnd,null_mut(),null(),&size,self.dc,&origin,0,&blend,ULW_ALPHA)==0 { return Err(std::io::Error::last_os_error().to_string()); }
        Result::Ok(())
    }
}
impl Drop for Canvas { fn drop(&mut self) { unsafe {
    if !self.family.is_null(){ GdipDeleteFontFamily(self.family); }
    if !self.g.is_null(){GdipDeleteGraphics(self.g);}
    if !self.image.is_null(){GdipDisposeImage(self.image.cast());}
    if !self.old.is_null(){SelectObject(self.dc,self.old);}
    if !self.bitmap.is_null(){DeleteObject(self.bitmap);}
    if !self.dc.is_null(){DeleteDC(self.dc);}
} } }

#[derive(Clone)] pub struct Row { pub id:u64, pub y:f32, pub height:f32 }
pub struct View<'a> { pub doc:&'a Document, pub scale:f32, pub archive:bool, pub scroll:f32,
    pub hover:Option<u64>, pub selected:Option<u64>, pub status:&'a str, pub editing:bool, pub dragging:Option<u64> }
pub struct Layout { pub rows:Vec<Row>, pub max_scroll:f32 }
pub fn draw(hwnd:HWND, view:View<'_>)->Result<Layout,String> { unsafe {
    let mut bounds:RECT=zeroed(); GetClientRect(hwnd,&mut bounds);
    let w=bounds.right as f32/view.scale; let h=bounds.bottom as f32/view.scale;
    let c=Canvas::new(bounds.right.max(1),bounds.bottom.max(1),view.scale)?;
    let light=view.doc.settings.light;
    let palette=Palette::new(light);
    let text=palette.text; let muted=palette.muted; let accent=palette.accent;
    let bg=(view.doc.settings.opacity as u32)<<24 | palette.background;
    c.round(2.,3.,w-4.,h-5.,if view.doc.settings.collapsed{20.}else{18.},bg);
    let mut rows=vec![]; let mut max_scroll=0.;
    if view.doc.settings.collapsed {
        c.round(15.,16.,8.,8.,4.,accent);
        c.text(&format!("近期 · {}",view.doc.pending()),32.,10.,w-48.,25.,14.,text,true);
    } else {
        c.text("L I T E L I S T",20.,18.,170.,18.,10.,muted,true);
        c.text(if view.archive{"已完成"}else{"近期要做"},20.,44.,w-110.,34.,23.,text,true);
        c.text(&format!("{:02}",if view.archive{view.doc.visible(true).len()}else{view.doc.pending()}),w-67.,44.,48.,36.,24.,accent,true);
        // Native-looking pin, menu dots, collapse chevron; avoid font-dependent symbols.
        c.round(w-100.,20.,7.,7.,3.5,if view.doc.settings.topmost{accent}else{muted});
        for dx in [0.,4.,8.] { c.round(w-72.+dx,22.,2.,2.,1.,muted); }
        c.line(w-38.,21.,w-33.,26.,muted,1.5); c.line(w-33.,26.,w-28.,21.,muted,1.5);
        c.line(20.,88.,w-20.,88.,palette.divider,1.);
        let list_bottom=h-106.;
        GdipSetClipRect(c.g,12.,96.,w-24.,(list_bottom-96.).max(0.),CombineModeReplace);
        let mut content_y=0.;
        for t in view.doc.visible(view.archive) {
            let text_height=c.measure(&t.text,w-102.);
            let rh=(text_height+23.).max(49.)+if t.reminder.as_ref().is_some_and(|r|r.enabled){22.}else{0.};
            let y=98.+content_y-view.scroll;
            rows.push(Row{id:t.id,y,height:rh});
            if y+rh>96. && y<list_bottom {
                if view.hover==Some(t.id)||view.selected==Some(t.id) {
                    c.round(12.,y,w-24.,rh-4.,9.,palette.hover);
                }
                let cy=y+15.;
                if view.archive {
                    c.round(23.,cy,17.,17.,5.,accent);
                    c.line(27.,cy+8.,30.,cy+11.,palette.checkmark,1.7);
                    c.line(30.,cy+11.,36.,cy+5.,palette.checkmark,1.7);
                } else {
                    // A translucent fill and an opaque light outline stay legible
                    // over wallpaper without the old opaque black inset.
                    c.round(23.,cy,17.,17.,4.5,palette.checkbox_fill);
                    c.outline(23.75,cy+0.75,15.5,15.5,4.,
                        if view.hover==Some(t.id){accent}else{palette.checkbox_border},1.5);
                }
                c.text(&t.text,51.,y+11.,w-102.,text_height+5.,14.,if view.archive{muted}else{text},false);
                if let Some(r)=t.reminder.as_ref().filter(|r|r.enabled){
                    let due=crate::reminders::format_local(r.due_at);
                    let label=format!("提醒 {}{}",due.get(5..).unwrap_or(&due),if r.early_at.is_some(){" · 含提前提醒"}else{""});
                    c.text(&label,51.,y+text_height+15.,w-72.,22.,10.,accent,false);
                }
                if view.hover==Some(t.id) { for dy in [0.,4.,8.] { c.line(w-34.,cy+dy,w-26.,cy+dy,muted,1.); } }
                if view.dragging==Some(t.id) { c.line(16.,y,w-16.,y,accent,2.); }
            }
            content_y+=rh;
        }
        max_scroll=(content_y-(list_bottom-98.)).max(0.);
        if max_scroll>0. { let track=list_bottom-100.; let thumb=(track*track/content_y).max(22.); let pos=(view.scroll/max_scroll).clamp(0.,1.)*(track-thumb); c.round(w-9.,100.+pos,3.,thumb,1.5,0x558da79b); }
        if rows.is_empty() {
            c.round(w/2.-23.,120.,46.,46.,14.,if light{0x204e9e7c}else{0x205eae8c});
            c.line(w/2.-9.,142.,w/2.-2.,149.,accent,2.); c.line(w/2.-2.,149.,w/2.+11.,134.,accent,2.);
            c.text(if view.archive{"完成的事情，会留在这里"}else{"把脑海里的事，放在这里"},27.,183.,w-54.,30.,14.,muted,false);
            c.text(if view.archive{"点击复选框可以恢复待办"}else{"从一件小事开始。"},27.,214.,w-54.,24.,12.,muted,false);
        }
        GdipResetClip(c.g);
        c.round(17.,h-96.,w-34.,45.,11.,palette.input);
        if !view.editing {
            c.line(30.,h-74.,42.,h-74.,accent,1.5); c.line(36.,h-80.,36.,h-68.,accent,1.5);
            let placeholder=if view.doc.settings.draft.is_empty(){"添加待办，回车记下"}else{&view.doc.settings.draft};
            c.text(placeholder,51.,h-85.,w-78.,28.,13.,muted,false);
        }
        c.text(if view.archive{"‹ 返回待办"}else{"查看已完成"},21.,h-37.,115.,21.,11.,muted,false);
        c.text(view.status,135.,h-37.,w-155.,21.,10.,muted,false);
        c.line(w-15.,h-15.,w-21.,h-9.,muted,1.); c.line(w-15.,h-20.,w-26.,h-9.,muted,1.);
    }
    c.present(hwnd)?;
    Result::Ok(Layout{rows,max_scroll})
} }

