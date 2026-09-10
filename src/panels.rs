//! Native, selectable text panels for notes, reminder settings and notification inbox.
#![allow(unsafe_op_in_unsafe_fn)]
use crate::{
    model::{Goal, GoalNode, Note, Task},
    render::wide,
};
use std::{
    cell::RefCell,
    mem::zeroed,
    ptr::{null, null_mut},
};
use windows_sys::Win32::System::SystemServices::SS_LEFTNOWORDWRAP;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{Diagnostics::Debug::MessageBeep, LibraryLoader::*},
    UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
pub const PANEL_EVENT: u32 = WM_APP + 20;
#[derive(Clone, Copy)]
pub enum Kind {
    Note,
    Reminder,
    Alert,
    Goal,
    GoalNode,
}
pub enum Event {
    Changed(HWND),
    Closed(HWND),
    DeleteNote(HWND),
    SaveReminder(HWND),
    Dismiss(HWND),
    Pin(HWND),
    SaveGoal(HWND),
    DeleteGoal(HWND),
    SaveGoalNode(HWND),
    DeleteGoalNode(HWND),
}
thread_local! {static QUEUE:RefCell<Vec<Event>>=const{RefCell::new(Vec::new())};}
pub fn drain() -> Vec<Event> {
    QUEUE.with(|q| std::mem::take(&mut *q.borrow_mut()))
}
struct Context {
    main: HWND,
    kind: Kind,
    font: HFONT,
    brush: HBRUSH,
    scale: f32,
}
unsafe fn signal(c: &Context, event: Event) {
    QUEUE.with(|q| q.borrow_mut().push(event));
    PostMessageW(c.main, PANEL_EVENT, 0, 0);
}
unsafe fn toggle_reminder_controls(hwnd: HWND) {
    let enabled = SendMessageW(GetDlgItem(hwnd, 103), BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
    for id in [111, 112, 113, 114, 104] {
        EnableWindow(GetDlgItem(hwnd, id), enabled as i32);
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = &*(l as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Context;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let c = &*ptr;
    match msg {
        WM_SIZE => {
            layout(hwnd, c);
            return 0;
        }
        WM_COMMAND => {
            let id = w & 0xffff;
            let code = (w >> 16) & 0xffff;
            if code == EN_CHANGE as usize && matches!(c.kind, Kind::Note) {
                SetTimer(hwnd, 1, 450, None);
            }
            if code == BN_CLICKED as usize {
                match id {
                    103 => toggle_reminder_controls(hwnd),
                    201 => match c.kind {
                        Kind::Reminder => signal(c, Event::SaveReminder(hwnd)),
                        Kind::Goal => signal(c, Event::SaveGoal(hwnd)),
                        Kind::GoalNode => signal(c, Event::SaveGoalNode(hwnd)),
                        _ => {}
                    },
                    202 => signal(c, Event::Closed(hwnd)),
                    203 => signal(c, Event::Dismiss(hwnd)),
                    204 => signal(c, Event::Pin(hwnd)),
                    205 => {
                        let (question, title) = match c.kind {
                            Kind::Note => ("删除这张便签？此操作无法撤销。", "删除便签"),
                            Kind::Goal => {
                                ("删除这个大目标及其全部节点？此操作无法撤销。", "删除大目标")
                            }
                            Kind::GoalNode => {
                                ("删除这个小目标？仍被依赖的节点无法删除。", "删除小目标")
                            }
                            _ => ("", ""),
                        };
                        if MessageBoxW(
                            hwnd,
                            wide(question).as_ptr(),
                            wide(title).as_ptr(),
                            MB_YESNO | MB_ICONWARNING,
                        ) == IDYES
                        {
                            match c.kind {
                                Kind::Note => signal(c, Event::DeleteNote(hwnd)),
                                Kind::Goal => signal(c, Event::DeleteGoal(hwnd)),
                                Kind::GoalNode => signal(c, Event::DeleteGoalNode(hwnd)),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            return 0;
        }
        WM_TIMER if w == 1 => {
            KillTimer(hwnd, 1);
            signal(c, Event::Changed(hwnd));
            return 0;
        }
        WM_EXITSIZEMOVE => {
            signal(c, Event::Changed(hwnd));
            return 0;
        }
        WM_CLOSE => {
            signal(
                c,
                if matches!(c.kind, Kind::Alert) {
                    Event::Dismiss(hwnd)
                } else {
                    Event::Closed(hwnd)
                },
            );
            return 0;
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC | WM_CTLCOLORLISTBOX => {
            let dc = w as HDC;
            let note = matches!(c.kind, Kind::Note);
            SetTextColor(dc, if note { 0x00313242 } else { 0x00262e36 });
            SetBkColor(dc, if note { 0x00f1f7fb } else { 0x00f7f8fa });
            return c.brush as isize;
        }
        WM_DPICHANGED => {
            let r = *(l as *const RECT);
            let c = &mut *ptr;
            c.scale = GetDpiForWindow(hwnd) as f32 / 96.;
            let font = make_font(c.scale);
            let old = c.font;
            c.font = font;
            let mut child = GetWindow(hwnd, GW_CHILD);
            while !child.is_null() {
                SendMessageW(child, WM_SETFONT, font as usize, 1);
                child = GetWindow(child, GW_HWNDNEXT);
            }
            DeleteObject(old);
            SetWindowPos(
                hwnd,
                null_mut(),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            layout(hwnd, &*ptr);
            return 0;
        }
        WM_GETMINMAXINFO => {
            let m = &mut *(l as *mut MINMAXINFO);
            let s = c.scale;
            m.ptMinTrackSize = POINT {
                x: (300. * s) as i32,
                y: (match c.kind {
                    Kind::Reminder => 355.,
                    Kind::Goal => 360.,
                    Kind::GoalNode => 500.,
                    _ => 250.,
                } * s) as i32,
            };
            return 0;
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            let c = Box::from_raw(ptr);
            DeleteObject(c.font);
            DeleteObject(c.brush);
            return DefWindowProcW(hwnd, msg, w, l);
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, w, l)
}
unsafe fn make_font(scale: f32) -> HFONT {
    CreateFontW(
        -(15. * scale) as i32,
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
    )
}
unsafe fn control(hwnd: HWND, class: &str, text: &str, id: usize, style: u32) -> HWND {
    let c = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Context);
    let child = CreateWindowExW(
        0,
        wide(class).as_ptr(),
        wide(text).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        0,
        0,
        10,
        10,
        hwnd,
        id as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    SendMessageW(child, WM_SETFONT, c.font as usize, 1);
    SetWindowTheme(child, wide("Explorer").as_ptr(), null());
    child
}
unsafe fn layout(hwnd: HWND, c: &Context) {
    let mut r: RECT = zeroed();
    GetClientRect(hwnd, &mut r);
    let s = c.scale;
    let w = r.right as f32 / s;
    let h = r.bottom as f32 / s;
    let place = |id: i32, x: f32, y: f32, width: f32, height: f32| {
        let child = GetDlgItem(hwnd, id);
        if !child.is_null() {
            MoveWindow(
                child,
                (x * s) as i32,
                (y * s) as i32,
                (width.max(1.) * s) as i32,
                (height.max(1.) * s) as i32,
                1,
            );
        }
    };
    match c.kind {
        Kind::Note => {
            place(101, 16., 16., w - 166., 31.);
            place(204, w - 142., 16., 58., 31.);
            place(205, w - 76., 16., 60., 31.);
            place(102, 16., 60., w - 32., h - 76.);
        }
        Kind::Reminder => {
            place(301, 18., 14., w - 36., 26.);
            place(103, 18., 48., w - 36., 26.);
            place(302, 18., 83., w - 36., 22.);
            place(111, 18., 107., w - 146., 30.);
            place(112, w - 120., 107., 102., 30.);
            place(303, 18., 150., w - 36., 22.);
            place(113, 18., 174., w - 146., 30.);
            place(114, w - 120., 174., 102., 30.);
            place(304, 18., 217., w - 36., 22.);
            place(104, 18., 241., w - 36., 30.);
            place(201, w - 202., h - 47., 88., 31.);
            place(202, w - 104., h - 47., 86., 31.);
        }
        Kind::Alert => {
            place(301, 18., 12., w - 36., 26.);
            place(102, 18., 47., w - 36., h - 110.);
            place(203, w - 128., h - 47., 110., 31.);
        }
        Kind::Goal => {
            place(301, 18., 16., w - 36., 22.);
            place(101, 18., 40., w - 36., 31.);
            place(302, 18., 84., w - 36., 22.);
            place(102, 18., 108., w - 36., h - 178.);
            place(205, 18., h - 47., 82., 31.);
            place(201, w - 202., h - 47., 88., 31.);
            place(202, w - 104., h - 47., 86., 31.);
        }
        Kind::GoalNode => {
            place(301, 18., 16., w - 36., 22.);
            place(101, 18., 40., w - 36., 31.);
            place(302, 18., 84., w - 36., 22.);
            place(102, 18., 108., w - 36., 72.);
            place(303, 18., 193., w - 36., 22.);
            place(105, 18., 217., w - 36., h - 320.);
            place(304, 18., h - 91., 70., 22.);
            place(106, 92., h - 95., 150., 29.);
            place(205, 18., h - 47., 82., 31.);
            place(201, w - 202., h - 47., 88., 31.);
            place(202, w - 104., h - 47., 86., 31.);
        }
    }
}
unsafe fn create(
    main: HWND,
    kind: Kind,
    title: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    top: bool,
) -> Result<HWND, String> {
    let instance = GetModuleHandleW(null());
    let class = wide("LiteListNativePanelV4");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(proc),
        hInstance: instance,
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
        lpszClassName: class.as_ptr(),
        ..zeroed()
    };
    RegisterClassW(&wc);
    let scale = GetDpiForWindow(main).max(96) as f32 / 96.;
    let context = Box::new(Context {
        main,
        kind,
        font: make_font(scale),
        brush: CreateSolidBrush(if matches!(kind, Kind::Note) {
            0x00f1f7fb
        } else {
            0x00f7f8fa
        }),
        scale,
    });
    let p = Box::into_raw(context);
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | if top { WS_EX_TOPMOST } else { 0 },
        class.as_ptr(),
        wide(title).as_ptr(),
        WS_OVERLAPPEDWINDOW,
        x,
        y,
        (w as f32 * scale) as i32,
        (h as f32 * scale) as i32,
        null_mut(),
        null_mut(),
        instance,
        p.cast(),
    );
    if hwnd.is_null() {
        drop(Box::from_raw(p));
        return Err("无法创建桌面面板".into());
    }
    Ok(hwnd)
}
unsafe fn show(hwnd: HWND, activate: bool) {
    let c = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Context);
    layout(hwnd, c);
    ShowWindow(
        hwnd,
        if activate {
            SW_SHOWNORMAL
        } else {
            SW_SHOWNOACTIVATE
        },
    );
    if activate {
        SetForegroundWindow(hwnd);
    }
}
unsafe fn picker(
    hwnd: HWND,
    id: usize,
    time: bool,
    value: Option<SYSTEMTIME>,
    optional: bool,
) -> HWND {
    let style = WS_TABSTOP
        | if time {
            DTS_TIMEFORMAT
        } else {
            DTS_SHORTDATECENTURYFORMAT
        }
        | if optional { DTS_SHOWNONE } else { 0 };
    let h = control(hwnd, "SysDateTimePick32", "", id, style);
    if let Some(value) = value {
        SendMessageW(
            h,
            DTM_SETSYSTEMTIME,
            GDT_VALID as usize,
            &value as *const SYSTEMTIME as isize,
        );
    } else if optional {
        SendMessageW(h, DTM_SETSYSTEMTIME, GDT_NONE as usize, 0);
    }
    h
}
pub unsafe fn note(main: HWND, n: &Note) -> Result<HWND, String> {
    let hwnd = create(
        main,
        Kind::Note,
        "LiteList 便签",
        n.x,
        n.y,
        n.width,
        n.height,
        n.topmost,
    )?;
    control(
        hwnd,
        "EDIT",
        &n.title,
        101,
        WS_TABSTOP | ES_AUTOHSCROLL as u32,
    );
    control(
        hwnd,
        "BUTTON",
        if n.topmost { "已置顶" } else { "置顶" },
        204,
        WS_TABSTOP | BS_FLAT as u32,
    );
    control(hwnd, "BUTTON", "删除", 205, WS_TABSTOP | BS_FLAT as u32);
    let edit = control(
        hwnd,
        "EDIT",
        &n.body.replace('\n', "\r\n").replace("\r\r\n", "\r\n"),
        102,
        WS_TABSTOP
            | WS_VSCROLL
            | ES_MULTILINE as u32
            | ES_AUTOVSCROLL as u32
            | ES_WANTRETURN as u32,
    );
    SendMessageW(edit, EM_SETLIMITTEXT, 200000, 0);
    SendMessageW(GetDlgItem(hwnd, 101), EM_SETLIMITTEXT, 150, 0);
    show(hwnd, false);
    Ok(hwnd)
}
pub unsafe fn reminder(main: HWND, t: &Task) -> Result<HWND, String> {
    let hwnd = create(
        main,
        Kind::Reminder,
        "提醒设置 · LiteList",
        180,
        140,
        460,
        375,
        true,
    )?;
    control(hwnd, "STATIC", &t.text, 301, SS_LEFTNOWORDWRAP);
    let checkbox = control(
        hwnd,
        "BUTTON",
        "启用提醒",
        103,
        WS_TABSTOP | BS_AUTOCHECKBOX as u32,
    );
    let reminder = t.reminder.as_ref();
    let enabled = reminder.is_some_and(|r| r.enabled);
    SendMessageW(
        checkbox,
        BM_SETCHECK,
        if enabled {
            BST_CHECKED as usize
        } else {
            BST_UNCHECKED as usize
        },
        0,
    );
    let due = reminder.and_then(|r| crate::reminders::local_time(r.due_at));
    let early = reminder
        .and_then(|r| r.early_at)
        .and_then(crate::reminders::local_time);
    let mut now = SYSTEMTIME::default();
    if due.is_none() {
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut now);
    }
    let due = due.unwrap_or(now);
    control(hwnd, "STATIC", "到点提醒", 302, 0);
    picker(hwnd, 111, false, Some(due), false);
    picker(hwnd, 112, true, Some(due), false);
    control(hwnd, "STATIC", "提前提醒（可选）", 303, 0);
    picker(hwnd, 113, false, early, true);
    picker(hwnd, 114, true, early, true);
    control(hwnd, "STATIC", "提醒备注（可选）", 304, 0);
    control(
        hwnd,
        "EDIT",
        &reminder.map(|r| r.early_note.clone()).unwrap_or_default(),
        104,
        WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
    );
    control(
        hwnd,
        "BUTTON",
        "保存",
        201,
        WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
    );
    control(
        hwnd,
        "BUTTON",
        "取消",
        202,
        WS_TABSTOP | BS_PUSHBUTTON as u32,
    );
    toggle_reminder_controls(hwnd);
    show(hwnd, true);
    Ok(hwnd)
}
pub unsafe fn alert(main: HWND, text: &str) -> Result<HWND, String> {
    let hwnd = create(
        main,
        Kind::Alert,
        "待办提醒 · LiteList",
        220,
        160,
        450,
        330,
        true,
    )?;
    control(hwnd, "STATIC", "有待办需要你的注意", 301, 0);
    control(
        hwnd,
        "EDIT",
        text,
        102,
        WS_TABSTOP | WS_VSCROLL | ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32,
    );
    control(
        hwnd,
        "BUTTON",
        "知道了",
        203,
        WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
    );
    show(hwnd, false);
    MessageBeep(MB_ICONINFORMATION);
    Ok(hwnd)
}

pub unsafe fn goal(main: HWND, value: Option<&Goal>) -> Result<HWND, String> {
    let hwnd = create(
        main,
        Kind::Goal,
        if value.is_some() {
            "编辑大目标 · LiteList"
        } else {
            "新建大目标 · LiteList"
        },
        210,
        150,
        480,
        390,
        true,
    )?;
    control(hwnd, "STATIC", "大目标名称", 301, 0);
    control(
        hwnd,
        "EDIT",
        value.map(|goal| goal.title.as_str()).unwrap_or(""),
        101,
        WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
    );
    control(hwnd, "STATIC", "说明（可选）", 302, 0);
    control(
        hwnd,
        "EDIT",
        value.map(|goal| goal.description.as_str()).unwrap_or(""),
        102,
        WS_TABSTOP
            | WS_BORDER
            | WS_VSCROLL
            | ES_MULTILINE as u32
            | ES_AUTOVSCROLL as u32
            | ES_WANTRETURN as u32,
    );
    let delete = control(
        hwnd,
        "BUTTON",
        "删除",
        205,
        WS_TABSTOP | BS_PUSHBUTTON as u32,
    );
    EnableWindow(delete, value.is_some() as i32);
    control(
        hwnd,
        "BUTTON",
        "保存",
        201,
        WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
    );
    control(
        hwnd,
        "BUTTON",
        "取消",
        202,
        WS_TABSTOP | BS_PUSHBUTTON as u32,
    );
    SendMessageW(GetDlgItem(hwnd, 101), EM_SETLIMITTEXT, 200, 0);
    SendMessageW(GetDlgItem(hwnd, 102), EM_SETLIMITTEXT, 2000, 0);
    show(hwnd, true);
    SetFocus(GetDlgItem(hwnd, 101));
    Ok(hwnd)
}

pub unsafe fn goal_node(main: HWND, goal: &Goal, value: Option<&GoalNode>) -> Result<HWND, String> {
    let hwnd = create(
        main,
        Kind::GoalNode,
        if value.is_some() {
            "编辑小目标 · LiteList"
        } else {
            "新建小目标 · LiteList"
        },
        230,
        120,
        500,
        540,
        true,
    )?;
    control(hwnd, "STATIC", "小目标名称", 301, 0);
    control(
        hwnd,
        "EDIT",
        value.map(|node| node.title.as_str()).unwrap_or(""),
        101,
        WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
    );
    control(hwnd, "STATIC", "说明（可选）", 302, 0);
    control(
        hwnd,
        "EDIT",
        value.map(|node| node.description.as_str()).unwrap_or(""),
        102,
        WS_TABSTOP
            | WS_BORDER
            | WS_VSCROLL
            | ES_MULTILINE as u32
            | ES_AUTOVSCROLL as u32
            | ES_WANTRETURN as u32,
    );
    control(
        hwnd,
        "STATIC",
        if value.is_some() {
            "前置条件（编辑时保持不变）"
        } else {
            "前置条件（可多选）"
        },
        303,
        0,
    );
    let list = control(
        hwnd,
        "LISTBOX",
        "",
        105,
        WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_MULTIPLESEL as u32 | LBS_NOINTEGRALHEIGHT as u32,
    );
    for node in &goal.nodes {
        if value.is_some_and(|current| current.id == node.id) {
            continue;
        }
        let index = SendMessageW(list, LB_ADDSTRING, 0, wide(&node.title).as_ptr() as isize);
        if index >= 0 {
            SendMessageW(list, LB_SETITEMDATA, index as usize, node.id as isize);
            if value.is_some_and(|current| current.requires.contains(&node.id)) {
                SendMessageW(list, LB_SETSEL, 1, index);
            }
        }
    }
    if value.is_some() {
        EnableWindow(list, 0);
    }
    control(hwnd, "STATIC", "节点形状", 304, 0);
    let combo = control(
        hwnd,
        "COMBOBOX",
        "",
        106,
        WS_TABSTOP | CBS_DROPDOWNLIST as u32,
    );
    for label in ["继承全局设置", "圆角", "直角", "胶囊"] {
        SendMessageW(combo, CB_ADDSTRING, 0, wide(label).as_ptr() as isize);
    }
    SendMessageW(
        combo,
        CB_SETCURSEL,
        value
            .map(|node| (node.shape + 1).clamp(0, 3) as usize)
            .unwrap_or(0),
        0,
    );
    let delete = control(
        hwnd,
        "BUTTON",
        "删除",
        205,
        WS_TABSTOP | BS_PUSHBUTTON as u32,
    );
    EnableWindow(delete, value.is_some() as i32);
    control(
        hwnd,
        "BUTTON",
        "保存",
        201,
        WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
    );
    control(
        hwnd,
        "BUTTON",
        "取消",
        202,
        WS_TABSTOP | BS_PUSHBUTTON as u32,
    );
    SendMessageW(GetDlgItem(hwnd, 101), EM_SETLIMITTEXT, 200, 0);
    SendMessageW(GetDlgItem(hwnd, 102), EM_SETLIMITTEXT, 2000, 0);
    show(hwnd, true);
    SetFocus(GetDlgItem(hwnd, 101));
    Ok(hwnd)
}
pub unsafe fn text(hwnd: HWND, id: i32) -> String {
    let h = GetDlgItem(hwnd, id);
    let n = GetWindowTextLengthW(h).clamp(0, 200000);
    let mut b = vec![0u16; n as usize + 1];
    let n = GetWindowTextW(h, b.as_mut_ptr(), b.len() as i32).max(0);
    String::from_utf16_lossy(&b[..n as usize])
}
unsafe fn picker_value(hwnd: HWND, id: i32) -> Option<SYSTEMTIME> {
    let mut value = SYSTEMTIME::default();
    if SendMessageW(
        GetDlgItem(hwnd, id),
        DTM_GETSYSTEMTIME,
        0,
        &mut value as *mut SYSTEMTIME as isize,
    ) == GDT_VALID as isize
    {
        Some(value)
    } else {
        None
    }
}
pub unsafe fn reminder_values(
    hwnd: HWND,
) -> Result<(bool, SYSTEMTIME, Option<SYSTEMTIME>, String), String> {
    let enabled = SendMessageW(GetDlgItem(hwnd, 103), BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
    let date = picker_value(hwnd, 111).ok_or("请选择到点日期")?;
    let time = picker_value(hwnd, 112).ok_or("请选择到点时间")?;
    let due = SYSTEMTIME {
        wYear: date.wYear,
        wMonth: date.wMonth,
        wDay: date.wDay,
        wHour: time.wHour,
        wMinute: time.wMinute,
        ..Default::default()
    };
    let early_date = picker_value(hwnd, 113);
    let early_time = picker_value(hwnd, 114);
    let early = match (early_date, early_time) {
        (None, None) => None,
        (Some(d), Some(t)) => Some(SYSTEMTIME {
            wYear: d.wYear,
            wMonth: d.wMonth,
            wDay: d.wDay,
            wHour: t.wHour,
            wMinute: t.wMinute,
            ..Default::default()
        }),
        _ => return Err("提前提醒请同时选择日期和时间".into()),
    };
    Ok((enabled, due, early, text(hwnd, 104)))
}
pub unsafe fn goal_values(hwnd: HWND) -> (String, String) {
    (text(hwnd, 101), text(hwnd, 102))
}
pub unsafe fn goal_node_values(hwnd: HWND) -> (String, String, Vec<u64>, i8) {
    let list = GetDlgItem(hwnd, 105);
    let count = SendMessageW(list, LB_GETCOUNT, 0, 0).max(0) as usize;
    let mut requires = vec![];
    for index in 0..count {
        if SendMessageW(list, LB_GETSEL, index, 0) > 0 {
            let id = SendMessageW(list, LB_GETITEMDATA, index, 0);
            if id > 0 {
                requires.push(id as u64);
            }
        }
    }
    let shape = SendMessageW(GetDlgItem(hwnd, 106), CB_GETCURSEL, 0, 0).max(0) as i8 - 1;
    (text(hwnd, 101), text(hwnd, 102), requires, shape)
}
pub unsafe fn note_values(hwnd: HWND, n: &mut Note) {
    n.title = text(hwnd, 101);
    n.body = text(hwnd, 102);
    if IsIconic(hwnd) != 0 {
        return;
    }
    let mut r: RECT = zeroed();
    GetWindowRect(hwnd, &mut r);
    let s = GetDpiForWindow(hwnd).max(96) as f32 / 96.;
    n.x = r.left;
    n.y = r.top;
    n.width = ((r.right - r.left) as f32 / s) as i32;
    n.height = ((r.bottom - r.top) as f32 / s) as i32;
}
pub unsafe fn is_dialog_message(msg: &MSG) -> bool {
    let root = GetAncestor(msg.hwnd, GA_ROOT);
    let mut name = [0u16; 80];
    let len = GetClassNameW(root, name.as_mut_ptr(), 80);
    if String::from_utf16_lossy(&name[..len.max(0) as usize]) == "LiteListNativePanelV4" {
        return IsDialogMessageW(root, msg) != 0;
    }
    false
}
