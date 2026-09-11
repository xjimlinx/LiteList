//! Event-driven premultiplied-alpha drawing. All native drawing resources are scoped.
#![allow(unsafe_op_in_unsafe_fn)]
use crate::model::Document;
use std::collections::{HashMap, HashSet};
use std::{
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Gdi::*, GdiPlus::*},
    UI::WindowsAndMessaging::*,
};

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
                background: 0x00f4f7f8,
                text: 0xff24343d,
                muted: 0xff52656c,
                accent: 0xff267956,
                divider: 0x20394d57,
                hover: 0x143d7965,
                input: 0x1643545f,
                checkbox_fill: 0xd9ffffff,
                checkbox_border: 0xff73878e,
                checkmark: 0xfff7fffb,
            }
        } else {
            Self {
                background: 0x001c2832,
                text: 0xfff3f6f8,
                muted: 0xffc2ccd3,
                accent: 0xffa8e6cb,
                divider: 0x1fffffff,
                hover: 0x14ffffff,
                input: 0x12ffffff,
                checkbox_fill: 0x16ffffff,
                checkbox_border: 0xffe9f0f4,
                checkmark: 0xff1c3c30,
            }
        }
    }
}
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub struct GdiSession(usize);
impl GdiSession {
    pub fn new() -> Result<Self, String> {
        unsafe {
            let mut token = 0;
            let input = GdiplusStartupInput {
                GdiplusVersion: 1,
                DebugEventCallback: 0,
                SuppressBackgroundThread: 0,
                SuppressExternalCodecs: 1,
            };
            if GdiplusStartup(&mut token, &input, null_mut()) != 0 {
                return Err("无法初始化系统绘图库".into());
            }
            Result::Ok(Self(token))
        }
    }
}
impl Drop for GdiSession {
    fn drop(&mut self) {
        unsafe {
            GdiplusShutdown(self.0);
        }
    }
}

struct Canvas {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    image: *mut GpBitmap,
    g: *mut GpGraphics,
    family: *mut GpFontFamily,
    width: i32,
    height: i32,
}
impl Canvas {
    unsafe fn new(width: i32, height: i32, scale: f32) -> Result<Self, String> {
        let dc = CreateCompatibleDC(null_mut());
        let mut c = Self {
            dc,
            bitmap: null_mut(),
            old: null_mut(),
            image: null_mut(),
            g: null_mut(),
            family: null_mut(),
            width,
            height,
        };
        if dc.is_null() {
            return Err("无法创建绘图上下文".into());
        }
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut bits = null_mut();
        c.bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if c.bitmap.is_null() {
            return Err("无法创建透明位图".into());
        }
        c.old = SelectObject(dc, c.bitmap);
        // PixelFormat32bppPARGB: the memory format required by UpdateLayeredWindow.
        if GdipCreateBitmapFromScan0(
            width,
            height,
            width * 4,
            0x000e200b,
            bits.cast(),
            &mut c.image,
        ) != 0
            || GdipGetImageGraphicsContext(c.image.cast(), &mut c.g) != 0
        {
            return Err("无法初始化透明画布".into());
        }
        GdipGraphicsClear(c.g, 0);
        GdipSetSmoothingMode(c.g, SmoothingModeAntiAlias);
        GdipSetTextRenderingHint(c.g, TextRenderingHintAntiAliasGridFit);
        GdipScaleWorldTransform(c.g, scale, scale, MatrixOrderPrepend);
        if GdipCreateFontFamilyFromName(
            wide("Microsoft YaHei UI").as_ptr(),
            null_mut(),
            &mut c.family,
        ) != 0
        {
            if GdipGetGenericFontFamilySansSerif(&mut c.family) != 0 {
                return Err("无法加载系统字体".into());
            }
        }
        Result::Ok(c)
    }
    unsafe fn round(&self, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32) {
        let mut p = null_mut();
        let mut b = null_mut();
        GdipCreatePath(FillModeAlternate, &mut p);
        GdipCreateSolidFill(color, &mut b);
        let d = r * 2.;
        GdipAddPathArc(p, x, y, d, d, 180., 90.);
        GdipAddPathArc(p, x + w - d, y, d, d, 270., 90.);
        GdipAddPathArc(p, x + w - d, y + h - d, d, d, 0., 90.);
        GdipAddPathArc(p, x, y + h - d, d, d, 90., 90.);
        GdipClosePathFigure(p);
        GdipFillPath(self.g, b.cast(), p);
        GdipDeletePath(p);
        GdipDeleteBrush(b.cast());
    }
    unsafe fn line(&self, x: f32, y: f32, x2: f32, y2: f32, color: u32, width: f32) {
        let mut p = null_mut();
        GdipCreatePen1(color, width, UnitPixel, &mut p);
        GdipDrawLine(self.g, p, x, y, x2, y2);
        GdipDeletePen(p);
    }
    unsafe fn outline(&self, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32, width: f32) {
        let mut path = null_mut();
        let mut pen = null_mut();
        GdipCreatePath(FillModeAlternate, &mut path);
        let d = r * 2.;
        GdipAddPathArc(path, x, y, d, d, 180., 90.);
        GdipAddPathArc(path, x + w - d, y, d, d, 270., 90.);
        GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0., 90.);
        GdipAddPathArc(path, x, y + h - d, d, d, 90., 90.);
        GdipClosePathFigure(path);
        GdipCreatePen1(color, width, UnitPixel, &mut pen);
        GdipDrawPath(self.g, pen, path);
        GdipDeletePen(pen);
        GdipDeletePath(path);
    }
    unsafe fn text(
        &self,
        s: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        size: f32,
        color: u32,
        bold: bool,
    ) {
        let mut f = null_mut();
        let mut b = null_mut();
        let mut fmt = null_mut();
        GdipCreateFont(
            self.family,
            size,
            if bold {
                FontStyleBold
            } else {
                FontStyleRegular
            },
            UnitPixel,
            &mut f,
        );
        GdipCreateSolidFill(color, &mut b);
        GdipCreateStringFormat(0, 0, &mut fmt);
        GdipSetStringFormatTrimming(fmt, StringTrimmingEllipsisCharacter);
        let s = wide(s);
        let rect = RectF {
            X: x,
            Y: y,
            Width: w,
            Height: h,
        };
        GdipDrawString(
            self.g,
            s.as_ptr(),
            (s.len() - 1) as i32,
            f,
            &rect,
            fmt,
            b.cast(),
        );
        GdipDeleteStringFormat(fmt);
        GdipDeleteBrush(b.cast());
        GdipDeleteFont(f);
    }
    unsafe fn measure(&self, s: &str, w: f32) -> f32 {
        let mut f = null_mut();
        GdipCreateFont(self.family, 14., FontStyleRegular, UnitPixel, &mut f);
        let s = wide(s);
        let rect = RectF {
            X: 0.,
            Y: 0.,
            Width: w,
            Height: 1000.,
        };
        let mut bounds: RectF = zeroed();
        GdipMeasureString(
            self.g,
            s.as_ptr(),
            (s.len() - 1) as i32,
            f,
            &rect,
            null(),
            &mut bounds,
            null_mut(),
            null_mut(),
        );
        GdipDeleteFont(f);
        bounds.Height.clamp(22., 88.)
    }
    unsafe fn present(&self, hwnd: HWND) -> Result<(), String> {
        GdipFlush(self.g, FlushIntentionSync);
        let size = SIZE {
            cx: self.width,
            cy: self.height,
        };
        let origin = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        if UpdateLayeredWindow(
            hwnd,
            null_mut(),
            null(),
            &size,
            self.dc,
            &origin,
            0,
            &blend,
            ULW_ALPHA,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Result::Ok(())
    }
}
impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            if !self.family.is_null() {
                GdipDeleteFontFamily(self.family);
            }
            if !self.g.is_null() {
                GdipDeleteGraphics(self.g);
            }
            if !self.image.is_null() {
                GdipDisposeImage(self.image.cast());
            }
            if !self.old.is_null() {
                SelectObject(self.dc, self.old);
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
            if !self.dc.is_null() {
                DeleteDC(self.dc);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Tasks,
    Goals,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ShowTasks,
    ShowGoals,
    AddGoal,
    ToggleGoal(u64),
    AddNode(u64),
    EditGoal(u64),
    EditNode(u64, u64),
    CompleteNode(u64, u64),
}

#[derive(Clone)]
pub struct Hit {
    pub action: Action,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Hit {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Clone)]
pub struct Row {
    pub id: u64,
    pub y: f32,
    pub height: f32,
}

pub struct View<'a> {
    pub doc: &'a Document,
    pub scale: f32,
    pub page: Page,
    pub archive: bool,
    pub scroll: f32,
    pub goal_pan: f32,
    pub hover: Option<u64>,
    pub selected: Option<u64>,
    pub status: &'a str,
    pub editing: bool,
    pub dragging: Option<u64>,
    pub collapsed_goals: &'a HashSet<u64>,
}

pub struct Layout {
    pub rows: Vec<Row>,
    pub hits: Vec<Hit>,
    pub max_scroll: f32,
    pub max_goal_pan: f32,
}

pub fn draw(hwnd: HWND, view: View<'_>) -> Result<Layout, String> {
    unsafe {
        let mut bounds: RECT = zeroed();
        GetClientRect(hwnd, &mut bounds);
        let w = bounds.right as f32 / view.scale;
        let h = bounds.bottom as f32 / view.scale;
        let c = Canvas::new(bounds.right.max(1), bounds.bottom.max(1), view.scale)?;
        let palette = Palette::new(view.doc.settings.light);
        let text = palette.text;
        let muted = palette.muted;
        let accent = palette.accent;
        let bg = (view.doc.settings.opacity as u32) << 24 | palette.background;
        c.round(
            2.,
            3.,
            w - 4.,
            h - 5.,
            if view.doc.settings.collapsed {
                20.
            } else {
                18.
            },
            bg,
        );
        let mut rows = vec![];
        let mut hits = vec![];
        let mut max_scroll = 0.;
        let mut max_goal_pan: f32 = 0.;
        if view.doc.settings.collapsed {
            c.round(15., 16., 8., 8., 4., accent);
            c.text(
                &format!("近期 · {}", view.doc.pending()),
                32.,
                10.,
                w - 48.,
                25.,
                14.,
                text,
                true,
            );
        } else {
            c.text("L I T E L I S T", 20., 18., 170., 18., 10., muted, true);
            let title = if view.page == Page::Goals {
                "目标路线"
            } else if view.archive {
                "已完成"
            } else {
                "近期要做"
            };
            let count = if view.page == Page::Goals {
                view.doc.goals.len()
            } else if view.archive {
                view.doc.visible(true).len()
            } else {
                view.doc.pending()
            };
            c.text(title, 20., 40., w - 110., 30., 21., text, true);
            c.text(
                &format!("{count:02}"),
                w - 67.,
                40.,
                48.,
                32.,
                22.,
                accent,
                true,
            );
            c.round(
                w - 100.,
                20.,
                7.,
                7.,
                3.5,
                if view.doc.settings.topmost {
                    accent
                } else {
                    muted
                },
            );
            for dx in [0., 4., 8.] {
                c.round(w - 72. + dx, 22., 2., 2., 1., muted);
            }
            c.line(w - 38., 21., w - 33., 26., muted, 1.5);
            c.line(w - 33., 26., w - 28., 21., muted, 1.5);

            c.round(
                18.,
                76.,
                72.,
                30.,
                9.,
                if view.page == Page::Tasks {
                    palette.hover
                } else {
                    0x08000000
                },
            );
            c.round(
                96.,
                76.,
                72.,
                30.,
                9.,
                if view.page == Page::Goals {
                    palette.hover
                } else {
                    0x08000000
                },
            );
            c.text(
                "待办",
                38.,
                81.,
                48.,
                20.,
                12.,
                if view.page == Page::Tasks {
                    accent
                } else {
                    muted
                },
                view.page == Page::Tasks,
            );
            c.text(
                "目标",
                116.,
                81.,
                48.,
                20.,
                12.,
                if view.page == Page::Goals {
                    accent
                } else {
                    muted
                },
                view.page == Page::Goals,
            );
            hits.push(Hit {
                action: Action::ShowTasks,
                x: 18.,
                y: 76.,
                width: 72.,
                height: 30.,
            });
            hits.push(Hit {
                action: Action::ShowGoals,
                x: 96.,
                y: 76.,
                width: 72.,
                height: 30.,
            });
            c.line(20., 114., w - 20., 114., palette.divider, 1.);

            let content_bottom = if view.page == Page::Tasks {
                h - 106.
            } else {
                h - 48.
            };
            GdipSetClipRect(
                c.g,
                12.,
                122.,
                w - 24.,
                (content_bottom - 122.).max(0.),
                CombineModeReplace,
            );
            let mut content_y = 0.;
            if view.page == Page::Tasks {
                for task in view.doc.visible(view.archive) {
                    let text_height = c.measure(&task.text, w - 102.);
                    let row_height = (text_height + 23.).max(49.)
                        + if task.reminder.as_ref().is_some_and(|r| r.enabled) {
                            22.
                        } else {
                            0.
                        };
                    let y = 124. + content_y - view.scroll;
                    rows.push(Row {
                        id: task.id,
                        y,
                        height: row_height,
                    });
                    if y + row_height > 122. && y < content_bottom {
                        if view.hover == Some(task.id) || view.selected == Some(task.id) {
                            c.round(12., y, w - 24., row_height - 4., 9., palette.hover);
                        }
                        let cy = y + 15.;
                        if view.archive {
                            c.round(23., cy, 17., 17., 5., accent);
                            c.line(27., cy + 8., 30., cy + 11., palette.checkmark, 1.7);
                            c.line(30., cy + 11., 36., cy + 5., palette.checkmark, 1.7);
                        } else {
                            c.round(23., cy, 17., 17., 4.5, palette.checkbox_fill);
                            c.outline(
                                23.75,
                                cy + 0.75,
                                15.5,
                                15.5,
                                4.,
                                if view.hover == Some(task.id) {
                                    accent
                                } else {
                                    palette.checkbox_border
                                },
                                1.5,
                            );
                        }
                        c.text(
                            &task.text,
                            51.,
                            y + 11.,
                            w - 102.,
                            text_height + 5.,
                            14.,
                            if view.archive { muted } else { text },
                            false,
                        );
                        if let Some(reminder) = task.reminder.as_ref().filter(|r| r.enabled) {
                            let due = crate::reminders::format_local(reminder.due_at);
                            let label = format!(
                                "提醒 {}{}",
                                due.get(5..).unwrap_or(&due),
                                if reminder.early_at.is_some() {
                                    " · 含提前提醒"
                                } else {
                                    ""
                                }
                            );
                            c.text(
                                &label,
                                51.,
                                y + text_height + 15.,
                                w - 72.,
                                22.,
                                10.,
                                accent,
                                false,
                            );
                        }
                        if view.hover == Some(task.id) {
                            for dy in [0., 4., 8.] {
                                c.line(w - 34., cy + dy, w - 26., cy + dy, muted, 1.);
                            }
                        }
                        if view.dragging == Some(task.id) {
                            c.line(16., y, w - 16., y, accent, 2.);
                        }
                    }
                    content_y += row_height;
                }
            } else {
                let available_width = (w - 32.).max(120.);
                for goal in &view.doc.goals {
                    let y = 124. + content_y - view.scroll;
                    let progress = goal.progress();
                    let terminated = goal.terminated_at.is_some();
                    let collapsed = view.collapsed_goals.contains(&goal.id);
                    let header_height = 80.;
                    if y + header_height > 122. && y < content_bottom {
                        c.round(14., y, w - 28., header_height - 4., 12., palette.hover);
                        c.outline(
                            14.,
                            y,
                            w - 28.,
                            header_height - 4.,
                            12.,
                            palette.divider,
                            1.,
                        );
                        c.round(
                            22.,
                            y + 13.,
                            32.,
                            32.,
                            16.,
                            if goal.completed_at.is_some() {
                                accent
                            } else if terminated {
                                palette.divider
                            } else {
                                palette.input
                            },
                        );
                        c.text(
                            if goal.completed_at.is_some() {
                                "✓"
                            } else if terminated {
                                "×"
                            } else {
                                "◆"
                            },
                            31.,
                            y + 18.,
                            18.,
                            20.,
                            12.,
                            if goal.completed_at.is_some() {
                                palette.checkmark
                            } else if terminated {
                                muted
                            } else {
                                accent
                            },
                            true,
                        );
                        c.text(
                            &goal.title,
                            62.,
                            y + 6.,
                            (w - 182.).max(70.),
                            22.,
                            14.,
                            text,
                            true,
                        );
                        if !goal.description.is_empty() {
                            c.text(
                                &goal.description,
                                62.,
                                y + 28.,
                                (w - 182.).max(70.),
                                18.,
                                10.,
                                muted,
                                false,
                            );
                        }
                        c.text(
                            &format!(
                                "{}/{} · {}",
                                progress.completed,
                                progress.total,
                                if goal.completed_at.is_some() {
                                    "已完成"
                                } else if terminated {
                                    "已终止"
                                } else {
                                    "进行中"
                                }
                            ),
                            62.,
                            y + 48.,
                            (w - 182.).max(70.),
                            20.,
                            10.,
                            if goal.completed_at.is_some() {
                                accent
                            } else {
                                muted
                            },
                            false,
                        );
                        c.round(62., y + 68., (w - 86.).max(20.), 3., 1.5, palette.divider);
                        let progress_width = (w - 86.).max(20.) * progress.ratio;
                        if progress_width > 0.5 {
                            c.round(
                                62.,
                                y + 68.,
                                progress_width,
                                3.,
                                (progress_width / 2.).min(1.5),
                                accent,
                            );
                        }
                        c.text(
                            if collapsed { "展开" } else { "收起" },
                            w - 108.,
                            y + 12.,
                            42.,
                            22.,
                            10.,
                            muted,
                            false,
                        );
                        c.round(w - 61., y + 10., 39., 25., 8., palette.input);
                        c.text(
                            "+",
                            w - 48.,
                            y + 11.,
                            20.,
                            20.,
                            15.,
                            if terminated { muted } else { accent },
                            true,
                        );
                        hits.push(Hit {
                            action: Action::EditGoal(goal.id),
                            x: 58.,
                            y,
                            width: (w - 174.).max(52.),
                            height: 58.,
                        });
                        hits.push(Hit {
                            action: Action::ToggleGoal(goal.id),
                            x: w - 114.,
                            y,
                            width: 48.,
                            height: 44.,
                        });
                        if !terminated {
                            hits.push(Hit {
                                action: Action::AddNode(goal.id),
                                x: w - 64.,
                                y,
                                width: 46.,
                                height: 44.,
                            });
                        }
                    }
                    content_y += header_height;
                    if !collapsed {
                        if goal.nodes.is_empty() {
                            c.text(
                                if terminated {
                                    "目标已终止；从编辑面板恢复后可继续"
                                } else {
                                    "还没有小目标，点击右上角 + 添加起始节点"
                                },
                                24.,
                                y + header_height + 16.,
                                w - 48.,
                                28.,
                                12.,
                                muted,
                                false,
                            );
                            content_y += 64.;
                        } else {
                            let tree = goal.layout(142., 98., 24., 42.);
                            max_goal_pan = max_goal_pan.max((tree.width - available_width).max(0.));
                            let local_pan =
                                view.goal_pan.min((tree.width - available_width).max(0.));
                            let offset_x =
                                16. + ((available_width - tree.width) / 2.).max(0.) - local_pan;
                            let tree_y = y + header_height;
                            let positions: HashMap<u64, (f32, f32)> = tree
                                .nodes
                                .iter()
                                .map(|item| (item.node_id, (offset_x + item.x, tree_y + item.y)))
                                .collect();
                            for node in &goal.nodes {
                                let Some((target_x, target_y)) = positions.get(&node.id) else {
                                    continue;
                                };
                                for required in &node.requires {
                                    let Some((source_x, source_y)) = positions.get(required) else {
                                        continue;
                                    };
                                    let sx = *source_x + 71.;
                                    let sy = *source_y + 98.;
                                    let tx = *target_x + 71.;
                                    let middle = sy + (*target_y - sy) / 2.;
                                    c.line(sx, sy, sx, middle, palette.divider, 1.5);
                                    c.line(sx, middle, tx, middle, palette.divider, 1.5);
                                    c.line(tx, middle, tx, *target_y, palette.divider, 1.5);
                                }
                            }
                            for item in &tree.nodes {
                                let Some(node) = goal.find_node(item.node_id) else {
                                    continue;
                                };
                                let x = offset_x + item.x;
                                let node_y = tree_y + item.y;
                                if node_y + 98. < 122. || node_y > content_bottom {
                                    continue;
                                }
                                let unlocked = goal.node_unlocked(node.id);
                                let shape = if node.shape >= 0 {
                                    node.shape
                                } else {
                                    view.doc.settings.goal_node_shape
                                };
                                let radius = if shape == 1 {
                                    1.
                                } else if shape == 2 {
                                    49.
                                } else {
                                    12.
                                };
                                c.round(
                                    x,
                                    node_y,
                                    142.,
                                    98.,
                                    radius,
                                    if node.completed_at.is_some() {
                                        0x383da67a
                                    } else {
                                        palette.input
                                    },
                                );
                                c.outline(
                                    x,
                                    node_y,
                                    142.,
                                    98.,
                                    radius,
                                    if node.completed_at.is_some() {
                                        accent
                                    } else {
                                        palette.divider
                                    },
                                    1.,
                                );
                                c.text(
                                    &node.title,
                                    x + 10.,
                                    node_y + 8.,
                                    122.,
                                    30.,
                                    12.,
                                    if unlocked || node.completed_at.is_some() {
                                        text
                                    } else {
                                        muted
                                    },
                                    true,
                                );
                                if !node.description.is_empty() {
                                    c.text(
                                        &node.description,
                                        x + 10.,
                                        node_y + 39.,
                                        122.,
                                        18.,
                                        9.,
                                        muted,
                                        false,
                                    );
                                }
                                let label = if terminated {
                                    "已终止"
                                } else if node.completed_at.is_some() {
                                    "恢复"
                                } else if unlocked {
                                    "完成"
                                } else {
                                    "需前置"
                                };
                                c.round(
                                    x + 9.,
                                    node_y + 70.,
                                    58.,
                                    20.,
                                    7.,
                                    if !terminated && (unlocked || node.completed_at.is_some()) {
                                        palette.hover
                                    } else {
                                        0x08000000
                                    },
                                );
                                c.text(
                                    label,
                                    x + 19.,
                                    node_y + 72.,
                                    48.,
                                    17.,
                                    9.,
                                    if !terminated && (unlocked || node.completed_at.is_some()) {
                                        accent
                                    } else {
                                        muted
                                    },
                                    false,
                                );
                                if !terminated {
                                    hits.push(Hit {
                                        action: Action::EditNode(goal.id, node.id),
                                        x: x + 3.,
                                        y: node_y + 3.,
                                        width: 136.,
                                        height: 92.,
                                    });
                                    // Added after the card hit so reverse hit-testing gives the
                                    // explicit completion control priority over editing.
                                    hits.push(Hit {
                                        action: Action::CompleteNode(goal.id, node.id),
                                        x: x + 7.,
                                        y: node_y + 67.,
                                        width: 64.,
                                        height: 27.,
                                    });
                                }
                            }
                            content_y += tree.height + 18.;
                        }
                    }
                    content_y += 10.;
                }
            }
            max_scroll = (content_y - (content_bottom - 124.)).max(0.);
            if max_scroll > 0. {
                let track = (content_bottom - 126.).max(30.);
                let thumb = (track * track / content_y.max(track)).clamp(22., track);
                let pos = (view.scroll / max_scroll).clamp(0., 1.) * (track - thumb);
                c.round(w - 9., 124. + pos, 3., thumb, 1.5, 0x558da79b);
            }
            if view.page == Page::Goals && max_goal_pan > 0. {
                let track = (w - 48.).max(40.);
                let total = track + max_goal_pan;
                let thumb = (track * track / total).clamp(30., track);
                let position = (view.goal_pan / max_goal_pan).clamp(0., 1.) * (track - thumb);
                c.round(24., content_bottom - 6., track, 3., 1.5, palette.divider);
                c.round(24. + position, content_bottom - 7., thumb, 5., 2.5, accent);
            }
            if view.page == Page::Tasks && rows.is_empty() {
                c.round(
                    w / 2. - 23.,
                    140.,
                    46.,
                    46.,
                    14.,
                    if view.doc.settings.light {
                        0x204e9e7c
                    } else {
                        0x205eae8c
                    },
                );
                c.line(w / 2. - 9., 162., w / 2. - 2., 169., accent, 2.);
                c.line(w / 2. - 2., 169., w / 2. + 11., 154., accent, 2.);
                c.text(
                    if view.archive {
                        "完成的事情，会留在这里"
                    } else {
                        "把脑海里的事，放在这里"
                    },
                    27.,
                    203.,
                    w - 54.,
                    30.,
                    14.,
                    muted,
                    false,
                );
                c.text(
                    if view.archive {
                        "点击复选框可以恢复待办"
                    } else {
                        "从一件小事开始。"
                    },
                    27.,
                    234.,
                    w - 54.,
                    24.,
                    12.,
                    muted,
                    false,
                );
            } else if view.page == Page::Goals && view.doc.goals.is_empty() {
                c.text(
                    "建立你的第一条目标路线",
                    28.,
                    165.,
                    w - 56.,
                    30.,
                    15.,
                    text,
                    true,
                );
                c.text(
                    "大目标拆成有前置条件的小目标",
                    28.,
                    199.,
                    w - 56.,
                    28.,
                    12.,
                    muted,
                    false,
                );
            }
            GdipResetClip(c.g);
            if view.page == Page::Tasks {
                c.round(17., h - 96., w - 34., 45., 11., palette.input);
                if !view.editing {
                    c.line(30., h - 74., 42., h - 74., accent, 1.5);
                    c.line(36., h - 80., 36., h - 68., accent, 1.5);
                    let placeholder = if view.doc.settings.draft.is_empty() {
                        "添加待办，回车记下"
                    } else {
                        &view.doc.settings.draft
                    };
                    c.text(placeholder, 51., h - 85., w - 78., 28., 13., muted, false);
                }
                c.text(
                    if view.archive {
                        "‹ 返回待办"
                    } else {
                        "查看已完成"
                    },
                    21.,
                    h - 37.,
                    115.,
                    21.,
                    11.,
                    muted,
                    false,
                );
                c.text(view.status, 135., h - 37., w - 155., 21., 10., muted, false);
            } else {
                c.round(18., h - 39., 96., 27., 8., palette.input);
                c.text("+ 新建大目标", 29., h - 35., 86., 20., 10., accent, true);
                hits.push(Hit {
                    action: Action::AddGoal,
                    x: 16.,
                    y: h - 43.,
                    width: 104.,
                    height: 35.,
                });
                c.text(view.status, 126., h - 35., w - 146., 20., 10., muted, false);
            }
            c.line(w - 15., h - 15., w - 21., h - 9., muted, 1.);
            c.line(w - 15., h - 20., w - 26., h - 9., muted, 1.);
        }
        c.present(hwnd)?;
        Result::Ok(Layout {
            rows,
            hits,
            max_scroll,
            max_goal_pan,
        })
    }
}
