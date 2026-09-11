//! Win32 boundary. Window callbacks only enqueue owned events; the event loop
//! mutates App after DispatchMessage returns, avoiding reentrant Rust &mut aliases.
#![allow(unsafe_op_in_unsafe_fn)]
use crate::{
    model::Model,
    render::{self, Action, Hit, Page, Row, wide},
    storage,
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashSet, VecDeque},
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr::{null, null_mut},
};
use windows_sys::Win32::System::SystemServices::MK_LBUTTON;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::*, Registry::*, Threading::*},
    UI::{
        Controls::Dialogs::*, Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*,
        WindowsAndMessaging::*,
    },
};

const TRAY: u32 = WM_APP + 1;
const WAKE: u32 = WM_APP + 2;
const SUBMIT: u32 = WM_APP + 3;
const CANCEL: u32 = WM_APP + 4;
const DRAFT: u32 = WM_APP + 5;
const MENU: u32 = WM_APP + 6;
thread_local! {
    static EVENTS:RefCell<VecDeque<(u32,usize,isize)>>=const{RefCell::new(VecDeque::new())};
    static EDIT_ORIGINAL:Cell<isize>=const{Cell::new(0)};
    static COMPOSING:Cell<bool>=const{Cell::new(false)};
    static EDIT_EPOCH:Cell<usize>=const{Cell::new(1)};
    static MAIN:Cell<HWND>=const{Cell::new(null_mut())};
}
fn queue(msg: u32, w: usize, l: isize) {
    EVENTS.with(|q| q.borrow_mut().push_back((msg, w, l)));
}
pub fn error_box(message: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(message).as_ptr(),
            wide("LiteList").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            let mut p = zeroed();
            BeginPaint(hwnd, &mut p);
            EndPaint(hwnd, &p);
            queue(msg, w, l);
            return 0;
        }
        WM_CLOSE => {
            queue(msg, w, l);
            return 0;
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            return 0;
        }
        WM_DPICHANGED => {
            queue(msg, w, 0);
            return 0;
        }
        WM_LBUTTONDOWN
        | WM_LBUTTONUP
        | WM_MOUSEMOVE
        | WM_LBUTTONDBLCLK
        | WM_RBUTTONUP
        | WM_MOUSEWHEEL
        | WM_KEYDOWN
        | WM_HOTKEY
        | WM_TIMER
        | WM_DISPLAYCHANGE
        | WM_MOVE
        | WM_SIZE
        | WM_MOUSELEAVE
        | WM_CAPTURECHANGED
        | TRAY
        | WAKE
        | SUBMIT
        | CANCEL
        | DRAFT
        | MENU
        | crate::panels::PANEL_EVENT => {
            queue(msg, w, l);
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, w, l)
}
unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_IME_STARTCOMPOSITION {
        COMPOSING.with(|v| v.set(true));
    }
    if msg == WM_IME_ENDCOMPOSITION {
        COMPOSING.with(|v| v.set(false));
    }
    if msg == WM_KILLFOCUS {
        MAIN.with(|h| {
            PostMessageW(h.get(), CANCEL, EDIT_EPOCH.with(Cell::get), 1);
        });
    }
    if msg == WM_KEYDOWN && !COMPOSING.with(Cell::get) {
        if w == VK_RETURN as usize && GetKeyState(VK_SHIFT as i32) >= 0 {
            MAIN.with(|h| {
                PostMessageW(h.get(), SUBMIT, 0, 0);
            });
            return 0;
        }
        if w == VK_ESCAPE as usize {
            MAIN.with(|h| {
                PostMessageW(h.get(), CANCEL, EDIT_EPOCH.with(Cell::get), 0);
            });
            return 0;
        }
    }
    if msg == WM_CHAR && (w == 27 || (w == 13 && GetKeyState(VK_SHIFT as i32) >= 0)) {
        return 0;
    }
    if matches!(
        msg,
        WM_CHAR | WM_PASTE | WM_CUT | WM_CLEAR | WM_UNDO | WM_IME_ENDCOMPOSITION
    ) {
        MAIN.with(|h| {
            PostMessageW(h.get(), DRAFT, 0, 0);
        });
    }
    let old = EDIT_ORIGINAL.with(Cell::get);
    CallWindowProcW(
        Some(std::mem::transmute::<
            isize,
            unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
        >(old)),
        hwnd,
        msg,
        w,
        l,
    )
}

#[derive(Clone, Copy)]
enum Drag {
    Move {
        screen: POINT,
        rect: RECT,
    },
    Resize {
        screen: POINT,
        width: i32,
        height: i32,
    },
    Task(u64),
    GoalPan {
        screen_x: i32,
        start: f32,
    },
}
struct App {
    hwnd: HWND,
    edit: HWND,
    font: HFONT,
    model: Model,
    dir: PathBuf,
    scale: f32,
    archive: bool,
    page: Page,
    scroll: f32,
    max_scroll: f32,
    goal_pan: f32,
    max_goal_pan: f32,
    rows: Vec<Row>,
    hits: Vec<Hit>,
    collapsed_goals: HashSet<u64>,
    hover: Option<u64>,
    selected: Option<u64>,
    drag: Option<Drag>,
    moved: bool,
    editing: bool,
    editing_id: Option<u64>,
    status: String,
    dirty: bool,
    last_mouse: (i32, i32),
    taskbar_msg: u32,
    notes: Vec<(u64, HWND)>,
    reminder_panel: Option<(u64, HWND)>,
    alert: Option<HWND>,
    alert_count: usize,
    goal_panel: Option<(Option<u64>, HWND)>,
    goal_node_panel: Option<(u64, Option<u64>, HWND)>,
}
impl App {
    unsafe fn copy_task(&mut self) {
        use windows_sys::Win32::System::{DataExchange::*, Memory::*};
        let Some(t) = self
            .selected
            .and_then(|id| self.model.doc.tasks.iter().find(|t| t.id == id))
        else {
            return;
        };
        let value = wide(&t.text);
        let memory = GlobalAlloc(GMEM_MOVEABLE, value.len() * 2);
        if memory.is_null() {
            error_box("无法分配剪贴板内存");
            return;
        }
        let dest = GlobalLock(memory) as *mut u16;
        if dest.is_null() {
            GlobalFree(memory);
            return;
        }
        std::ptr::copy_nonoverlapping(value.as_ptr(), dest, value.len());
        GlobalUnlock(memory);
        if OpenClipboard(self.hwnd) == 0 {
            GlobalFree(memory);
            error_box("剪贴板暂时被占用，请重试");
            return;
        }
        if EmptyClipboard() == 0 || SetClipboardData(13, memory).is_null() {
            GlobalFree(memory);
            CloseClipboard();
            error_box("复制失败，请重试");
            return;
        }
        CloseClipboard();
        self.status = "已复制整条任务".into();
        self.render();
    }
    unsafe fn new_note(&mut self) {
        if self.model.doc.notes.len() >= 200 {
            error_box("便签已达到 200 张上限");
            return;
        }
        let id = self.model.doc.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let work = self.work_area();
        let note = crate::model::Note {
            id,
            x: work.left + 60 + (id % 6) as i32 * 24,
            y: work.top + 60 + (id % 6) as i32 * 24,
            ..Default::default()
        };
        match crate::panels::note(self.hwnd, &note) {
            Ok(h) => {
                self.notes.push((id, h));
                self.model.doc.notes.push(note);
                self.save();
                SetForegroundWindow(h);
                SetFocus(GetDlgItem(h, 102));
            }
            Err(e) => error_box(&e),
        }
    }
    unsafe fn show_notes(&mut self) {
        for n in &mut self.model.doc.notes {
            n.visible = true;
            if let Some((_, h)) = self.notes.iter().find(|(id, _)| *id == n.id) {
                ShowWindow(*h, SW_SHOWNORMAL);
            } else {
                match crate::panels::note(self.hwnd, n) {
                    Ok(h) => self.notes.push((n.id, h)),
                    Err(e) => error_box(&e),
                }
            }
        }
        self.save();
    }
    unsafe fn open_reminder(&mut self) {
        if let Some((_, h)) = self.reminder_panel {
            SetForegroundWindow(h);
            return;
        }
        if let Some(t) = self
            .selected
            .and_then(|id| self.model.doc.tasks.iter().find(|t| t.id == id))
        {
            match crate::panels::reminder(self.hwnd, t) {
                Ok(h) => self.reminder_panel = Some((t.id, h)),
                Err(e) => error_box(&e),
            }
        }
    }
    unsafe fn open_goal(&mut self, goal_id: Option<u64>) {
        if let Some((_, hwnd)) = self.goal_panel {
            SetForegroundWindow(hwnd);
            return;
        }
        let goal = goal_id.and_then(|id| self.model.doc.find_goal(id));
        match crate::panels::goal(self.hwnd, goal) {
            Ok(hwnd) => self.goal_panel = Some((goal_id, hwnd)),
            Err(error) => error_box(&error),
        }
    }
    unsafe fn open_goal_node(&mut self, goal_id: u64, node_id: Option<u64>) {
        if let Some((_, _, hwnd)) = self.goal_node_panel {
            SetForegroundWindow(hwnd);
            return;
        }
        let Some(goal) = self.model.doc.find_goal(goal_id) else {
            return;
        };
        let node = node_id.and_then(|id| goal.find_node(id));
        match crate::panels::goal_node(self.hwnd, goal, node) {
            Ok(hwnd) => self.goal_node_panel = Some((goal_id, node_id, hwnd)),
            Err(error) => error_box(&error),
        }
    }
    unsafe fn check_reminders(&mut self) {
        if self.model.collect_reminders(crate::model::now()) {
            self.save();
            self.render();
        }
        if !self.dirty {
            self.show_alerts();
        }
    }
    unsafe fn show_alerts(&mut self) {
        let pending: Vec<_> = self
            .model
            .doc
            .notifications
            .iter()
            .filter(|n| !n.acknowledged)
            .collect();
        if pending.is_empty() {
            return;
        }
        if self.alert.is_some() && self.alert_count == pending.len() {
            return;
        }
        let count = pending.len();
        let body = pending
            .iter()
            .map(|n| format!("{}\r\n{}", n.title, n.text))
            .collect::<Vec<_>>()
            .join("\r\n\r\n────────────\r\n\r\n");
        if let Some(h) = self.alert {
            SetWindowTextW(GetDlgItem(h, 102), wide(&body).as_ptr());
        } else {
            match crate::panels::alert(self.hwnd, &body) {
                Ok(h) => self.alert = Some(h),
                Err(e) => {
                    self.status = e;
                    return;
                }
            }
        }
        self.alert_count = count;
    }
    unsafe fn panel_events(&mut self) {
        use crate::panels::Event;
        for event in crate::panels::drain() {
            match event {
                Event::Changed(h) => {
                    if self.notes.iter().any(|(_, w)| *w == h) {
                        self.save();
                    }
                }
                Event::Closed(h) => {
                    if let Some(index) = self.notes.iter().position(|(_, w)| *w == h) {
                        self.save();
                        if self.dirty {
                            continue;
                        }
                        let (id, _) = self.notes.remove(index);
                        if let Some(n) = self.model.doc.notes.iter_mut().find(|n| n.id == id) {
                            n.visible = false;
                        }
                        DestroyWindow(h);
                        self.save();
                    } else if self.reminder_panel.is_some_and(|(_, w)| w == h) {
                        self.reminder_panel = None;
                        DestroyWindow(h);
                    } else if self.goal_panel.is_some_and(|(_, w)| w == h) {
                        self.goal_panel = None;
                        DestroyWindow(h);
                    } else if self.goal_node_panel.is_some_and(|(_, _, w)| w == h) {
                        self.goal_node_panel = None;
                        DestroyWindow(h);
                    }
                }
                Event::DeleteNote(h) => {
                    if let Some(index) = self.notes.iter().position(|(_, w)| *w == h) {
                        let (id, _) = self.notes.remove(index);
                        self.model.doc.notes.retain(|n| n.id != id);
                        DestroyWindow(h);
                        self.save();
                    }
                }
                Event::Pin(h) => {
                    if let Some((id, _)) = self.notes.iter().find(|(_, w)| *w == h) {
                        if let Some(n) = self.model.doc.notes.iter_mut().find(|n| n.id == *id) {
                            n.topmost = !n.topmost;
                            SetWindowPos(
                                h,
                                if n.topmost {
                                    HWND_TOPMOST
                                } else {
                                    HWND_NOTOPMOST
                                },
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                            SetWindowTextW(
                                GetDlgItem(h, 204),
                                wide(if n.topmost { "已置顶" } else { "置顶" }).as_ptr(),
                            );
                        }
                    }
                    self.save();
                }
                Event::Dismiss(h) => {
                    if self.alert == Some(h) {
                        for n in self
                            .model
                            .doc
                            .notifications
                            .iter_mut()
                            .filter(|n| !n.acknowledged)
                            .take(self.alert_count)
                        {
                            n.acknowledged = true;
                        }
                        self.alert = None;
                        self.alert_count = 0;
                        DestroyWindow(h);
                        self.save();
                    }
                }
                Event::SaveReminder(h) => {
                    let Some((id, panel)) = self.reminder_panel else {
                        continue;
                    };
                    if panel != h {
                        continue;
                    }
                    let Some(t) = self.model.doc.tasks.iter_mut().find(|t| t.id == id) else {
                        continue;
                    };
                    match crate::panels::reminder_values(h).and_then(
                        |(enabled, due, early, note)| {
                            crate::reminders::configured_time(
                                enabled,
                                due,
                                early,
                                &note,
                                t.reminder.as_ref(),
                            )
                        },
                    ) {
                        Ok(r) => {
                            t.reminder = Some(r);
                            t.reminder_configured = true;
                            t.schedule_checked = true;
                            self.save();
                            if !self.dirty {
                                self.reminder_panel = None;
                                DestroyWindow(h);
                                self.render();
                                self.check_reminders();
                            }
                        }
                        Err(e) => {
                            MessageBoxW(
                                h,
                                wide(&e).as_ptr(),
                                wide("请检查提醒时间").as_ptr(),
                                MB_OK | MB_ICONWARNING,
                            );
                        }
                    }
                }
                Event::SaveGoal(h) => {
                    let Some((goal_id, panel)) = self.goal_panel else {
                        continue;
                    };
                    if panel != h {
                        continue;
                    }
                    let (title, description) = crate::panels::goal_values(h);
                    let result = if let Some(id) = goal_id {
                        self.model
                            .doc
                            .update_goal(id, &title, &description)
                            .map(|_| id)
                    } else {
                        self.model.doc.add_goal(&title, &description)
                    };
                    match result {
                        Ok(id) => {
                            self.collapsed_goals.remove(&id);
                            self.goal_panel = None;
                            DestroyWindow(h);
                            self.status = "大目标已保存".into();
                            self.save();
                            self.render();
                        }
                        Err(error) => {
                            MessageBoxW(
                                h,
                                wide(&error).as_ptr(),
                                wide("无法保存大目标").as_ptr(),
                                MB_OK | MB_ICONWARNING,
                            );
                        }
                    }
                }
                Event::DeleteGoal(h) => {
                    let Some((Some(goal_id), panel)) = self.goal_panel else {
                        continue;
                    };
                    if panel != h {
                        continue;
                    }
                    match self.model.doc.delete_goal(goal_id) {
                        Ok(()) => {
                            self.collapsed_goals.remove(&goal_id);
                            self.goal_panel = None;
                            DestroyWindow(h);
                            self.status = "大目标已删除".into();
                            self.save();
                            self.render();
                        }
                        Err(error) => error_box(&error),
                    }
                }
                Event::ToggleGoalTermination(h) => {
                    let Some((Some(goal_id), panel)) = self.goal_panel else {
                        continue;
                    };
                    if panel != h {
                        continue;
                    }
                    match self
                        .model
                        .doc
                        .toggle_goal_termination(goal_id, crate::model::now())
                    {
                        Ok(terminated) => {
                            if terminated {
                                self.collapsed_goals.insert(goal_id);
                            } else {
                                self.collapsed_goals.remove(&goal_id);
                            }
                            self.goal_panel = None;
                            DestroyWindow(h);
                            self.status = if terminated {
                                "大目标已终止".into()
                            } else {
                                "大目标已恢复".into()
                            };
                            self.save();
                            self.render();
                        }
                        Err(error) => error_box(&error),
                    }
                }
                Event::SaveGoalNode(h) => {
                    let Some((goal_id, node_id, panel)) = self.goal_node_panel else {
                        continue;
                    };
                    if panel != h {
                        continue;
                    }
                    let (title, description, requires, shape) = crate::panels::goal_node_values(h);
                    let result = if let Some(id) = node_id {
                        self.model
                            .doc
                            .update_goal_node(goal_id, id, &title, &description, shape)
                            .map(|_| id)
                    } else {
                        self.model
                            .doc
                            .add_goal_node(goal_id, &title, &description, requires, shape)
                    };
                    match result {
                        Ok(_) => {
                            self.goal_node_panel = None;
                            DestroyWindow(h);
                            self.status = "小目标已保存".into();
                            self.save();
                            self.render();
                        }
                        Err(error) => {
                            MessageBoxW(
                                h,
                                wide(&error).as_ptr(),
                                wide("无法保存小目标").as_ptr(),
                                MB_OK | MB_ICONWARNING,
                            );
                        }
                    }
                }
                Event::DeleteGoalNode(h) => {
                    let Some((goal_id, Some(node_id), panel)) = self.goal_node_panel else {
                        continue;
                    };
                    if panel != h {
                        continue;
                    }
                    match self.model.doc.delete_goal_node(goal_id, node_id) {
                        Ok(()) => {
                            self.goal_node_panel = None;
                            DestroyWindow(h);
                            self.status = "小目标已删除".into();
                            self.save();
                            self.render();
                        }
                        Err(error) => {
                            MessageBoxW(
                                h,
                                wide(&error).as_ptr(),
                                wide("无法删除小目标").as_ptr(),
                                MB_OK | MB_ICONWARNING,
                            );
                        }
                    }
                }
            }
        }
    }
}
impl App {
    unsafe fn render(&mut self) {
        match render::draw(
            self.hwnd,
            render::View {
                doc: &self.model.doc,
                scale: self.scale,
                page: self.page,
                archive: self.archive,
                scroll: self.scroll,
                goal_pan: self.goal_pan,
                hover: self.hover,
                selected: self.selected,
                status: &self.status,
                editing: self.editing,
                dragging: match self.drag {
                    Some(Drag::Task(id)) => Some(id),
                    _ => None,
                },
                collapsed_goals: &self.collapsed_goals,
            },
        ) {
            Ok(layout) => {
                self.rows = layout.rows;
                self.hits = layout.hits;
                self.max_scroll = layout.max_scroll;
                self.max_goal_pan = layout.max_goal_pan;
                if self.scroll > self.max_scroll {
                    self.scroll = self.max_scroll;
                    self.render();
                }
                if self.goal_pan > self.max_goal_pan {
                    self.goal_pan = self.max_goal_pan;
                    self.render();
                }
            }
            Err(e) => {
                self.status = format!("绘制失败：{e}");
            }
        }
    }
    unsafe fn save(&mut self) {
        self.snapshot_position();
        let mut collapsed: Vec<u64> = self.collapsed_goals.iter().copied().collect();
        collapsed.sort_unstable();
        self.model.doc.settings.collapsed_goal_ids = collapsed;
        for (id, h) in &self.notes {
            if let Some(n) = self.model.doc.notes.iter_mut().find(|n| n.id == *id) {
                crate::panels::note_values(*h, n);
            }
        }
        match storage::save(&self.dir, &self.model.doc) {
            Ok(()) => {
                self.dirty = false;
                self.status = "已保存到本机".into();
            }
            Err(e) => {
                self.dirty = true;
                self.status = "保存失败，退出前请重试".into();
                error_box(&format!("待办仍保留在内存，请勿关闭程序。\n{e}"));
            }
        }
    }
    unsafe fn snapshot_position(&mut self) {
        let mut r: RECT = zeroed();
        GetWindowRect(self.hwnd, &mut r);
        self.model.doc.settings.x = r.left;
        self.model.doc.settings.y = r.top;
    }
    unsafe fn work_area(&self) -> RECT {
        let monitor = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            info.rcWork
        } else {
            RECT {
                left: 0,
                top: 0,
                right: GetSystemMetrics(SM_CXSCREEN),
                bottom: GetSystemMetrics(SM_CYSCREEN),
            }
        }
    }
    fn dimensions(&self) -> (i32, i32) {
        let s = &self.model.doc.settings;
        let (w, h) = if s.collapsed {
            (148, 42)
        } else {
            (s.width, s.height)
        };
        (
            (w as f32 * self.scale).round() as i32,
            (h as f32 * self.scale).round() as i32,
        )
    }
    unsafe fn place(&mut self) {
        let (w, h) = self.dimensions();
        let work = self.work_area();
        let s = &mut self.model.doc.settings;
        if s.x == -32000 {
            s.x = work.right - w - 28;
            s.y = work.top + 80;
        }
        s.x = s.x.clamp(work.left, (work.right - w).max(work.left));
        s.y = s.y.clamp(work.top, (work.bottom - h).max(work.top));
        SetWindowPos(
            self.hwnd,
            if s.topmost {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            },
            s.x,
            s.y,
            w,
            h,
            SWP_NOACTIVATE,
        );
        self.position_editor();
        self.render();
    }
    unsafe fn position_editor(&mut self) {
        if !self.editing {
            return;
        }
        let mut r: RECT = zeroed();
        GetWindowRect(self.hwnd, &mut r);
        let px = |v: f32| (v * self.scale).round() as i32;
        if let Some(id) = self.editing_id {
            if let Some(row) = self.rows.iter().find(|row| row.id == id) {
                SetWindowPos(
                    self.edit,
                    HWND_TOP,
                    r.left + px(49.),
                    r.top + px(row.y + 9.),
                    r.right - r.left - px(99.),
                    px((row.height - 12.).max(32.)),
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                return;
            }
        }
        SetWindowPos(
            self.edit,
            HWND_TOP,
            r.left + px(23.),
            r.bottom - px(90.),
            r.right - r.left - px(46.),
            px(34.),
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    unsafe fn editor_text(&self) -> String {
        let n = GetWindowTextLengthW(self.edit).clamp(0, 32000) as usize;
        let mut s = vec![0u16; n + 1];
        let len = GetWindowTextW(self.edit, s.as_mut_ptr(), s.len() as i32);
        String::from_utf16_lossy(&s[..len.max(0) as usize])
    }
    unsafe fn begin_edit(&mut self, id: Option<u64>) {
        if self.model.doc.settings.collapsed {
            self.model.doc.settings.collapsed = false;
            self.place();
        }
        if self.editing {
            self.end_edit(false);
        }
        EDIT_EPOCH.with(|v| v.set(v.get().wrapping_add(1)));
        self.editing_id = id;
        self.editing = true;
        let content = id
            .and_then(|id| {
                self.model
                    .doc
                    .tasks
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.text.clone())
            })
            .unwrap_or_else(|| self.model.doc.settings.draft.clone());
        SetWindowTextW(self.edit, wide(&content).as_ptr());
        self.position_editor();
        ShowWindow(self.edit, SW_SHOW);
        SetForegroundWindow(self.edit);
        SetFocus(self.edit);
        SendMessageW(
            self.edit,
            EM_SETSEL,
            if id.is_some() {
                0
            } else {
                content.encode_utf16().count()
            },
            -1,
        );
        self.status = if id.is_some() {
            "回车保存 · Esc 取消"
        } else {
            "回车添加 · 支持多行粘贴"
        }
        .into();
        self.render();
    }
    unsafe fn end_edit(&mut self, submit: bool) {
        if !self.editing {
            return;
        }
        let value = self.editor_text();
        if submit {
            if value.trim().is_empty() {
                return;
            }
            if let Some(id) = self.editing_id {
                self.model.edit(id, &value);
            } else {
                self.model.add(&value);
                self.model.doc.settings.draft.clear();
            }
        } else if self.editing_id.is_none() {
            self.model.doc.settings.draft = value;
        }
        let again = submit && self.editing_id.is_none();
        EDIT_EPOCH.with(|v| v.set(v.get().wrapping_add(1)));
        self.editing = false;
        self.editing_id = None;
        ShowWindow(self.edit, SW_HIDE);
        self.save();
        self.render();
        if again {
            self.archive = false;
            self.scroll = self.max_scroll;
            self.begin_edit(None);
        }
    }
    unsafe fn collapse(&mut self) {
        self.end_edit(false);
        self.model.doc.settings.collapsed = !self.model.doc.settings.collapsed;
        self.place();
        self.save();
        self.render();
    }
    unsafe fn tray(&self, action: u32) {
        let mut n: NOTIFYICONDATAW = zeroed();
        n.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        n.hWnd = self.hwnd;
        n.uID = 1;
        n.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        n.uCallbackMessage = TRAY;
        n.hIcon = LoadIconW(GetModuleHandleW(null()), 1usize as *const u16);
        if n.hIcon.is_null() {
            n.hIcon = LoadIconW(null_mut(), IDI_APPLICATION);
        }
        let tip = wide("轻单 LiteList · Ctrl+Alt+Space 唤出");
        n.szTip[..tip.len()].copy_from_slice(&tip);
        Shell_NotifyIconW(action, &n);
    }
    unsafe fn menu(&mut self, task: Option<u64>) {
        let menu = CreatePopupMenu();
        let add = |id: usize, text: &str, checked: bool| {
            AppendMenuW(
                menu,
                MF_STRING | if checked { MF_CHECKED } else { 0 },
                id,
                wide(text).as_ptr(),
            );
        };
        if let Some(id) = task {
            self.selected = Some(id);
            add(23, "复制整条任务", false);
            add(24, "提醒设置…", false);
            add(20, "编辑 / 选中文字", false);
            add(21, "完成 / 恢复任务", false);
            add(22, "删除任务（可撤销）", false);
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
        }
        add(30, "新建桌面便签", false);
        add(31, "显示全部便签", false);
        add(32, "查看未读提醒", false);
        add(1, "展开 / 收起", false);
        add(2, "始终置顶", self.model.doc.settings.topmost);
        add(3, "浅色主题", self.model.doc.settings.light);
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        for (id, label, alpha) in [
            (4, "背景不透明度 40%", 102),
            (5, "背景不透明度 65%", 166),
            (6, "背景不透明度 85%", 220),
            (7, "背景不透明度 100%", 255),
        ] {
            add(id, label, self.model.doc.settings.opacity == alpha);
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        for (id, label, shape) in [
            (40, "目标节点：圆角", 0),
            (41, "目标节点：直角", 1),
            (42, "目标节点：胶囊", 2),
        ] {
            add(id, label, self.model.doc.settings.goal_node_shape == shape);
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        add(8, "撤销上一步任务操作", false);
        add(9, "恢复最近删除的任务", false);
        add(10, "打开数据文件夹", false);
        add(11, "导出备份到文件夹", false);
        add(12, "导入 JSON 备份…", false);
        add(13, "开机启动", autostart_enabled());
        add(14, "使用帮助", false);
        add(15, "退出 LiteList", false);
        let mut p: POINT = zeroed();
        GetCursorPos(&mut p);
        SetForegroundWindow(self.hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            p.x,
            p.y,
            0,
            self.hwnd,
            null(),
        );
        DestroyMenu(menu);
        PostMessageW(self.hwnd, WM_NULL, 0, 0);
        if cmd != 0 {
            self.command(cmd as u32);
        }
    }
    unsafe fn command(&mut self, id: u32) {
        match id {
            1 => {
                self.collapse();
                return;
            }
            2 => {
                self.model.doc.settings.topmost = !self.model.doc.settings.topmost;
                self.place();
            }
            3 => self.model.doc.settings.light = !self.model.doc.settings.light,
            4..=7 => self.model.doc.settings.opacity = [102, 166, 220, 255][(id - 4) as usize],
            40..=42 => self.model.doc.settings.goal_node_shape = (id - 40) as i8,
            8 => {
                self.model.undo();
            }
            9 => {
                self.model.restore_deleted();
            }
            10 => {
                let _ = std::fs::create_dir_all(&self.dir);
                open_path(&self.dir);
                return;
            }
            11 => {
                let p = self.dir.join("exports");
                let result = std::fs::create_dir_all(&p)
                    .map_err(|e| e.to_string())
                    .and_then(|_| {
                        serde_json::to_vec_pretty(&self.model.doc).map_err(|e| e.to_string())
                    })
                    .and_then(|b| {
                        storage::atomic_write(
                            &p.join(format!("LiteList-{}.json", crate::model::now())),
                            &b,
                        )
                    });
                if let Err(e) = result {
                    error_box(&e);
                } else {
                    open_path(&p);
                }
                return;
            }
            12 => {
                self.import();
                return;
            }
            13 => {
                if let Err(e) = set_autostart(!autostart_enabled()) {
                    error_box(&e);
                }
                return;
            }
            14 => {
                MessageBoxW(self.hwnd,wide("轻单 LiteList 0.1\n\n点击底部：添加任务；回车连续添加\n双击任务：编辑；复选框：完成 / 恢复\n拖动右侧手柄：排序；右键：更多操作\n拖动顶栏：移动；右下角：缩放\n右上角箭头：胶囊展开 / 收起\nCtrl+Z：撤销任务操作（输入框内为文本撤销）\nCtrl+Alt+Space：唤出\n\n完全本地运行，无需登录。\n退出请使用右键或托盘菜单。\n数据与备份可从右键菜单打开。").as_ptr(),wide("关于轻单").as_ptr(),MB_OK);
                return;
            }
            15 => {
                self.exit();
                return;
            }
            20 => {
                if let Some(id) = self.selected {
                    self.begin_edit(Some(id));
                }
                return;
            }
            21 => {
                if let Some(id) = self.selected {
                    self.model.toggle(id);
                }
            }
            22 => {
                if let Some(id) = self.selected {
                    self.model.delete(id);
                }
            }
            23 => {
                self.copy_task();
                return;
            }
            24 => {
                self.open_reminder();
                return;
            }
            30 => {
                self.new_note();
                return;
            }
            31 => {
                self.show_notes();
                return;
            }
            32 => {
                self.show_alerts();
                return;
            }
            _ => return,
        }
        self.save();
        self.render();
    }
    unsafe fn import(&mut self) {
        let mut buffer = vec![0u16; 32768];
        let mut ofn: OPENFILENAMEW = zeroed();
        let filter = wide("JSON 备份\0*.json\0所有文件\0*.*\0");
        ofn.lStructSize = size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = self.hwnd;
        ofn.lpstrFile = buffer.as_mut_ptr();
        ofn.nMaxFile = buffer.len() as u32;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.Flags = OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR;
        if GetOpenFileNameW(&mut ofn) == 0 {
            return;
        }
        let n = buffer.iter().position(|v| *v == 0).unwrap_or(0);
        let path = PathBuf::from(String::from_utf16_lossy(&buffer[..n]));
        let result = std::fs::metadata(&path)
            .map_err(|e| e.to_string())
            .and_then(|m| {
                if m.len() > 32 * 1024 * 1024 {
                    Err("备份文件过大".into())
                } else {
                    std::fs::read(&path).map_err(|e| e.to_string())
                }
            })
            .and_then(|b| {
                serde_json::from_slice::<crate::model::Document>(&b).map_err(|e| e.to_string())
            })
            .and_then(|mut d| {
                d.validate()?;
                Ok(d)
            });
        match result {
            Ok(mut doc) => {
                if MessageBoxW(
                    self.hwnd,
                    wide("用此备份替换当前清单？\n当前清单会另存为导入前备份。").as_ptr(),
                    wide("导入备份").as_ptr(),
                    MB_YESNO | MB_ICONQUESTION,
                ) != IDYES
                {
                    return;
                }
                self.end_edit(false);
                let before = self
                    .dir
                    .join(format!("before-import-{}.json", crate::model::now()));
                let saved = serde_json::to_vec_pretty(&self.model.doc)
                    .map_err(|e| e.to_string())
                    .and_then(|b| storage::atomic_write(&before, &b));
                if let Err(e) = saved {
                    error_box(&e);
                    return;
                }
                doc.settings = self.model.doc.settings.clone();
                self.model = Model::new(doc);
                self.collapsed_goals
                    .retain(|id| self.model.doc.find_goal(*id).is_some());
                self.scroll = 0.;
                self.save();
                self.render();
            }
            Err(e) => error_box(&format!("无法导入：{e}")),
        }
    }
    unsafe fn exit(&mut self) {
        self.end_edit(false);
        self.save();
        if self.dirty {
            return;
        }
        for (_, h) in self.notes.drain(..) {
            DestroyWindow(h);
        }
        if let Some((_, h)) = self.reminder_panel.take() {
            DestroyWindow(h);
        }
        if let Some(h) = self.alert.take() {
            DestroyWindow(h);
        }
        if let Some((_, h)) = self.goal_panel.take() {
            DestroyWindow(h);
        }
        if let Some((_, _, h)) = self.goal_node_panel.take() {
            DestroyWindow(h);
        }
        self.tray(NIM_DELETE);
        UnregisterHotKey(self.hwnd, 1);
        DestroyWindow(self.edit);
        DestroyWindow(self.hwnd);
    }
    fn row_at(&self, x: f32, y: f32) -> Option<u64> {
        if self.model.doc.settings.collapsed
            || self.page != Page::Tasks
            || x < 15.
            || y < 122.
            || y > self.model.doc.settings.height as f32 - 106.
        {
            return None;
        }
        self.rows
            .iter()
            .find(|r| y >= r.y && y < r.y + r.height)
            .map(|r| r.id)
    }
    fn action_at(&self, x: f32, y: f32) -> Option<Action> {
        self.hits
            .iter()
            .rev()
            .find(|hit| hit.contains(x, y))
            .map(|hit| hit.action)
    }
    unsafe fn activate_action(&mut self, action: Action) {
        match action {
            Action::ShowTasks => {
                self.end_edit(false);
                self.page = Page::Tasks;
                self.scroll = 0.;
                self.goal_pan = 0.;
            }
            Action::ShowGoals => {
                self.end_edit(false);
                self.page = Page::Goals;
                self.scroll = 0.;
                self.goal_pan = 0.;
            }
            Action::AddGoal => {
                self.open_goal(None);
                return;
            }
            Action::ToggleGoal(goal_id) => {
                if !self.collapsed_goals.remove(&goal_id) {
                    self.collapsed_goals.insert(goal_id);
                }
                self.save();
            }
            Action::AddNode(goal_id) => {
                self.open_goal_node(goal_id, None);
                return;
            }
            Action::EditGoal(goal_id) => {
                self.open_goal(Some(goal_id));
                return;
            }
            Action::EditNode(goal_id, node_id) => {
                self.open_goal_node(goal_id, Some(node_id));
                return;
            }
            Action::CompleteNode(goal_id, node_id) => {
                match self
                    .model
                    .doc
                    .toggle_goal_node(goal_id, node_id, crate::model::now())
                {
                    Ok(true) => self.status = "小目标已完成".into(),
                    Ok(false) => self.status = "小目标已恢复".into(),
                    Err(error) => {
                        self.status = error;
                        self.render();
                        return;
                    }
                }
                self.save();
            }
        }
        self.render();
    }
    unsafe fn event(&mut self, msg: u32, w: usize, l: isize) {
        let x = (l as u16 as i16) as f32 / self.scale;
        let y = ((l as u32 >> 16) as u16 as i16) as f32 / self.scale;
        if msg == self.taskbar_msg {
            self.tray(NIM_ADD);
            return;
        }
        match msg {
            WM_PAINT => self.render(),
            WAKE | WM_HOTKEY => {
                ShowWindow(self.hwnd, SW_SHOWNORMAL);
                self.model.doc.settings.collapsed = false;
                self.page = Page::Tasks;
                self.place();
                self.begin_edit(None);
            }
            WM_CLOSE => {
                if !self.model.doc.settings.collapsed {
                    self.collapse();
                }
            }
            SUBMIT => self.end_edit(true),
            CANCEL => {
                if w == EDIT_EPOCH.with(Cell::get) && self.editing {
                    if l == 1 {
                        if let Some(id) = self.editing_id {
                            self.model.edit(id, &self.editor_text());
                        }
                    }
                    self.end_edit(false);
                }
            }
            crate::panels::PANEL_EVENT => self.panel_events(),
            WM_TIMER if w == 10 => self.check_reminders(),
            MENU => self.command(w as u32),
            DRAFT => {
                if self.editing && self.editing_id.is_none() {
                    self.model.doc.settings.draft = self.editor_text();
                    SetTimer(self.hwnd, 2, 700, None);
                }
            }
            WM_TIMER if w == 2 => {
                KillTimer(self.hwnd, 2);
                self.save();
                self.render();
            }
            WM_TIMER if w == 3 => {
                KillTimer(self.hwnd, 3);
                self.snapshot_position();
                self.save();
            }
            WM_DPICHANGED => {
                self.scale = GetDpiForWindow(self.hwnd) as f32 / 96.;
                self.refresh_font();
                self.place();
            }
            WM_DISPLAYCHANGE => {
                self.snapshot_position();
                self.place();
                self.save();
            }
            WM_MOVE => {
                self.position_editor();
            }
            WM_SIZE => self.render(),
            TRAY => {
                if l as u32 == WM_LBUTTONUP {
                    self.collapse();
                    ShowWindow(self.hwnd, SW_SHOW);
                } else if l as u32 == WM_RBUTTONUP {
                    self.menu(None);
                }
            }
            WM_RBUTTONUP => {
                self.end_edit(false);
                self.menu(self.row_at(x, y));
            }
            WM_LBUTTONDBLCLK => {
                if let Some(id) = self.row_at(x, y) {
                    if x > 45. && x < (self.model.doc.settings.width - 48) as f32 {
                        self.begin_edit(Some(id));
                    }
                }
            }
            WM_LBUTTONDOWN => {
                SetCapture(self.hwnd);
                self.moved = false;
                self.last_mouse = (x as i32, y as i32);
                let mut screen: POINT = zeroed();
                GetCursorPos(&mut screen);
                let mut rect: RECT = zeroed();
                GetWindowRect(self.hwnd, &mut rect);
                let s = &self.model.doc.settings;
                if s.collapsed || (y < 72. && !(y < 40. && x > s.width as f32 - 113.)) {
                    self.drag = Some(Drag::Move { screen, rect });
                } else if x > s.width as f32 - 25. && y > s.height as f32 - 25. {
                    self.drag = Some(Drag::Resize {
                        screen,
                        width: s.width,
                        height: s.height,
                    });
                } else if self.page == Page::Goals && y >= 122. && self.action_at(x, y).is_none() {
                    self.drag = Some(Drag::GoalPan {
                        screen_x: screen.x,
                        start: self.goal_pan,
                    });
                } else if x > s.width as f32 - 48. {
                    if let Some(id) = self.row_at(x, y) {
                        self.drag = Some(Drag::Task(id));
                    }
                }
                if let Some(id) = self.row_at(x, y) {
                    self.selected = Some(id);
                    if x > 46. && x < (self.model.doc.settings.width - 48) as f32 {
                        ReleaseCapture();
                        self.drag = None;
                        self.begin_edit(Some(id));
                        let mut p: POINT = zeroed();
                        GetCursorPos(&mut p);
                        ScreenToClient(self.edit, &mut p);
                        SendMessageW(
                            self.edit,
                            WM_LBUTTONDOWN,
                            MK_LBUTTON as usize,
                            ((p.y as u32) << 16 | (p.x as u32 & 0xffff)) as isize,
                        );
                    }
                }
            }
            WM_MOUSEMOVE => {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: self.hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut track);
                if let Some(drag) = self.drag {
                    let mut p: POINT = zeroed();
                    GetCursorPos(&mut p);
                    match drag {
                        Drag::Move { screen, rect } => {
                            let dx = p.x - screen.x;
                            let dy = p.y - screen.y;
                            if dx.abs() + dy.abs() > 4 {
                                self.moved = true;
                            }
                            if self.moved {
                                SetWindowPos(
                                    self.hwnd,
                                    null_mut(),
                                    rect.left + dx,
                                    rect.top + dy,
                                    0,
                                    0,
                                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                                );
                                self.position_editor();
                            }
                        }
                        Drag::Resize {
                            screen,
                            width,
                            height,
                        } => {
                            self.moved = true;
                            self.model.doc.settings.width = (width
                                + ((p.x - screen.x) as f32 / self.scale) as i32)
                                .clamp(260, 1000);
                            self.model.doc.settings.height = (height
                                + ((p.y - screen.y) as f32 / self.scale) as i32)
                                .clamp(280, 1200);
                            self.snapshot_position();
                            self.place();
                        }
                        Drag::Task(_) => {
                            self.moved = true;
                            let hover = self.row_at(x, y);
                            if self.hover != hover {
                                self.hover = hover;
                                self.render();
                            }
                        }
                        Drag::GoalPan { screen_x, start } => {
                            let dx = (p.x - screen_x) as f32 / self.scale;
                            if dx.abs() > 4. {
                                self.moved = true;
                            }
                            self.goal_pan = (start - dx).clamp(0., self.max_goal_pan);
                            self.render();
                        }
                    }
                } else {
                    let hover = self.row_at(x, y);
                    if self.hover != hover {
                        self.hover = hover;
                        self.render();
                    }
                }
            }
            WM_MOUSELEAVE => {
                if self.drag.is_none() {
                    self.hover = None;
                    self.render();
                }
            }
            WM_LBUTTONUP => {
                let drag = self.drag.take();
                ReleaseCapture();
                if self.moved {
                    if let Some(Drag::Task(from)) = drag {
                        if let Some(to) = self.row_at(x, y) {
                            self.model.reorder(from, to);
                        }
                    }
                    if !matches!(drag, Some(Drag::GoalPan { .. })) {
                        self.snapshot_position();
                        self.place();
                        self.save();
                    }
                    self.render();
                    return;
                }
                if self.model.doc.settings.collapsed {
                    self.collapse();
                    return;
                }
                let width = self.model.doc.settings.width as f32;
                let height = self.model.doc.settings.height as f32;
                if y < 40. && x > width - 49. {
                    self.collapse();
                } else if y < 40. && x > width - 82. {
                    self.menu(None);
                } else if y < 40. && x > width - 113. {
                    self.command(2);
                } else if let Some(action) = self.action_at(x, y) {
                    self.activate_action(action);
                } else if let Some(id) = self.row_at(x, y) {
                    self.selected = Some(id);
                    if x < 46. {
                        self.end_edit(false);
                        self.model.toggle(id);
                        self.save();
                    }
                    self.render();
                } else if self.page == Page::Tasks && y > height - 97. && y < height - 49. {
                    self.begin_edit(None);
                } else if self.page == Page::Tasks && y > height - 45. && x < 130. {
                    self.end_edit(false);
                    self.archive = !self.archive;
                    self.scroll = 0.;
                    self.selected = None;
                    self.render();
                }
            }
            WM_CAPTURECHANGED => {
                self.drag = None;
            }
            WM_MOUSEWHEEL => {
                if !self.model.doc.settings.collapsed {
                    let delta = ((w >> 16) as u16 as i16) as f32;
                    if self.page == Page::Goals && GetKeyState(VK_SHIFT as i32) < 0 {
                        self.goal_pan =
                            (self.goal_pan - delta / 120. * 65.).clamp(0., self.max_goal_pan);
                    } else {
                        self.scroll = (self.scroll - delta / 120. * 65.).clamp(0., self.max_scroll);
                    }
                    self.render();
                }
            }
            WM_KEYDOWN => {
                if w == VK_ESCAPE as usize {
                    if !self.model.doc.settings.collapsed {
                        self.collapse();
                    }
                } else if w == b'C' as usize && GetKeyState(VK_CONTROL as i32) < 0 {
                    self.copy_task();
                } else if w == b'Z' as usize && GetKeyState(VK_CONTROL as i32) < 0 {
                    self.command(8);
                } else if w == VK_DELETE as usize {
                    self.command(22);
                } else if w == VK_F2 as usize {
                    self.command(20);
                } else if w == VK_RETURN as usize && self.page == Page::Tasks {
                    self.begin_edit(None);
                }
            }
            _ => {}
        }
    }
    unsafe fn refresh_font(&mut self) {
        let old = self.font;
        self.font = CreateFontW(
            -(15. * self.scale) as i32,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Microsoft YaHei UI").as_ptr(),
        );
        SendMessageW(self.edit, WM_SETFONT, self.font as usize, 1);
        if !old.is_null() {
            DeleteObject(old);
        }
    }
}

unsafe fn open_path(path: &std::path::Path) {
    ShellExecuteW(
        null_mut(),
        wide("open").as_ptr(),
        wide(&path.to_string_lossy()).as_ptr(),
        null(),
        null(),
        SW_SHOWNORMAL,
    );
}
unsafe fn autostart_enabled() -> bool {
    let mut key = null_mut();
    if RegOpenKeyExW(
        HKEY_CURRENT_USER,
        wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run").as_ptr(),
        0,
        KEY_QUERY_VALUE,
        &mut key,
    ) != 0
    {
        return false;
    }
    let status = RegQueryValueExW(
        key,
        wide("LiteList").as_ptr(),
        null(),
        null_mut(),
        null_mut(),
        null_mut(),
    );
    RegCloseKey(key);
    status == 0
}
unsafe fn set_autostart(enable: bool) -> Result<(), String> {
    let mut key = null_mut();
    let result = RegCreateKeyExW(
        HKEY_CURRENT_USER,
        wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run").as_ptr(),
        0,
        null(),
        0,
        KEY_SET_VALUE,
        null(),
        &mut key,
        null_mut(),
    );
    if result != 0 {
        return Err(format!("无法设置开机启动：{result}"));
    }
    let result = if enable {
        let value = wide(&format!(
            "\"{}\"",
            std::env::current_exe()
                .map_err(|e| e.to_string())?
                .display()
        ));
        RegSetValueExW(
            key,
            wide("LiteList").as_ptr(),
            0,
            REG_SZ,
            value.as_ptr().cast(),
            (value.len() * 2) as u32,
        )
    } else {
        RegDeleteValueW(key, wide("LiteList").as_ptr())
    };
    RegCloseKey(key);
    if result != 0 {
        return Err(format!("无法更新开机启动：{result}"));
    }
    Ok(())
}

pub fn run() -> Result<(), String> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let controls = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_DATE_CLASSES | ICC_STANDARD_CLASSES,
        };
        InitCommonControlsEx(&controls);
        let dir = storage::data_dir()?;
        // Isolate developer/test instances by data directory, not by executable name.
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        dir.to_string_lossy().to_lowercase().hash(&mut hash);
        let class = wide(&format!("LiteListWindow-{:x}", hash.finish()));
        let mutex = CreateMutexW(
            null(),
            0,
            wide(&format!("Local\\LiteList-{:x}", hash.finish())).as_ptr(),
        );
        if mutex.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let existing = FindWindowW(class.as_ptr(), null());
            if !existing.is_null() {
                if std::env::args().any(|a| a == "--quit") {
                    PostMessageW(existing, MENU, 15, 0);
                } else {
                    PostMessageW(existing, WAKE, 0, 0);
                }
            }
            CloseHandle(mutex);
            return Ok(());
        }
        if std::env::args().any(|a| a == "--quit") {
            CloseHandle(mutex);
            return Ok(());
        }
        let _gdi = render::GdiSession::new()?;
        let (doc, notice) = storage::load(&dir)?;
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hIcon: LoadIconW(instance, 1usize as *const u16),
            lpszClassName: class.as_ptr(),
            ..zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let visible_test = std::env::args().any(|a| a == "--test-window");
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED
                | if visible_test {
                    WS_EX_APPWINDOW
                } else {
                    WS_EX_TOOLWINDOW
                },
            class.as_ptr(),
            wide("轻单 LiteList").as_ptr(),
            WS_POPUP,
            100,
            100,
            340,
            440,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        MAIN.with(|v| v.set(hwnd));
        // A small owned native EDIT popup renders correctly alongside per-pixel alpha.
        let edit = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            wide("EDIT").as_ptr(),
            wide("添加待办").as_ptr(),
            WS_POPUP | WS_BORDER | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32,
            0,
            0,
            280,
            34,
            hwnd,
            null_mut(),
            instance,
            null(),
        );
        if edit.is_null() {
            return Err("无法创建中文输入控件".into());
        }
        EDIT_ORIGINAL.with(|p| {
            p.set(SetWindowLongPtrW(
                edit,
                GWLP_WNDPROC,
                edit_proc as *const () as isize,
            ))
        });
        SendMessageW(edit, EM_SETLIMITTEXT, 10000, 0);
        let scale = GetDpiForWindow(hwnd) as f32 / 96.;
        let initial_page = if std::env::args().any(|argument| argument == "--test-goals") {
            Page::Goals
        } else {
            Page::Tasks
        };
        let collapsed_goals = doc.settings.collapsed_goal_ids.iter().copied().collect();
        let mut app = App {
            hwnd,
            edit,
            font: null_mut(),
            model: Model::new(doc),
            dir,
            scale,
            archive: false,
            page: initial_page,
            scroll: 0.,
            max_scroll: 0.,
            goal_pan: 0.,
            max_goal_pan: 0.,
            rows: vec![],
            hits: vec![],
            collapsed_goals,
            hover: None,
            selected: None,
            drag: None,
            moved: false,
            editing: false,
            editing_id: None,
            status: "本地保存 · 随手记下".into(),
            dirty: false,
            last_mouse: (0, 0),
            taskbar_msg: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            notes: vec![],
            reminder_panel: None,
            alert: None,
            alert_count: 0,
            goal_panel: None,
            goal_node_panel: None,
        };
        app.refresh_font();
        app.place();
        app.tray(NIM_ADD);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        app.render();
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        if visible_test {
            let _ = std::fs::create_dir_all(&app.dir);
            let mut r: RECT = zeroed();
            GetWindowRect(hwnd, &mut r);
            let _ = std::fs::write(
                app.dir.join("startup-diagnostics.txt"),
                format!(
                    "visible={} rect={},{},{},{} status={} hwnd={:?}",
                    IsWindowVisible(hwnd),
                    r.left,
                    r.top,
                    r.right,
                    r.bottom,
                    app.status,
                    hwnd
                ),
            );
        }
        if RegisterHotKey(
            hwnd,
            1,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            VK_SPACE as u32,
        ) == 0
        {
            app.status = "快捷键被占用，可从托盘唤出".into();
            app.render();
        }
        if let Some(notice) = notice {
            MessageBoxW(
                hwnd,
                wide(&notice).as_ptr(),
                wide("数据恢复").as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        for n in app.model.doc.notes.clone().iter().filter(|n| n.visible) {
            match crate::panels::note(hwnd, n) {
                Ok(h) => app.notes.push((n.id, h)),
                Err(e) => error_box(&e),
            }
        }
        app.save();
        SetTimer(hwnd, 10, 1000, None);
        let mut msg: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            if msg.message == app.taskbar_msg {
                app.tray(NIM_ADD);
            }
            if !crate::panels::is_dialog_message(&msg) {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            loop {
                let event = EVENTS.with(|q| q.borrow_mut().pop_front());
                if let Some((m, w, l)) = event {
                    app.event(m, w, l);
                } else {
                    break;
                }
            }
        }
        if !app.font.is_null() {
            DeleteObject(app.font);
        }
        CloseHandle(mutex);
        Ok(())
    }
}
