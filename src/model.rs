//! Safe, platform-independent task model. Deleted items remain recoverable.
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub deleted_at: Option<u64>,
    #[serde(default)] pub reminder: Option<crate::reminders::Reminder>,
    /// Old releases inferred reminders from text. Only explicit settings survive migration.
    #[serde(default)] pub reminder_configured: bool,
    #[serde(default)] pub schedule_checked: bool,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(default)]
pub struct Note {pub id:u64,pub title:String,pub body:String,pub x:i32,pub y:i32,pub width:i32,pub height:i32,pub visible:bool,pub topmost:bool}
impl Default for Note {fn default()->Self{Self{id:0,title:"桌面便签".into(),body:String::new(),x:120,y:120,width:360,height:360,visible:true,topmost:false}}}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub collapsed: bool,
    pub topmost: bool,
    pub opacity: u8,
    pub light: bool,
    pub draft: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self { x: -32000, y: -32000, width: 340, height: 440,
            collapsed: false, topmost: true, opacity: 220, light: false, draft: String::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub next_id: u64,
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)] pub notes: Vec<Note>,
    #[serde(default)] pub notifications: Vec<crate::reminders::Notification>,
}
impl Default for Document {
    fn default() -> Self { Self { version: 2, next_id: 1, tasks: vec![], settings: Settings::default(),notes:vec![],notifications:vec![] } }
}
impl Document {
    pub fn validate(&mut self) -> Result<(), String> {
        if ![1,2].contains(&self.version) { return Err(format!("不支持的数据版本 {}，请使用对应版本的 LiteList", self.version)); }
        self.version=2;
        if self.tasks.len() > 100_000 { return Err("任务数量超过安全读取上限".into()); }
        let mut ids = std::collections::HashSet::new();
        for t in &self.tasks {
            if t.text.len() > 100_000 || t.text.contains('\0') || !ids.insert(t.id) {
                return Err("任务数据包含无效内容或重复 ID".into());
            }
            if let Some(r)=&t.reminder {if r.due_at>253402214400000||r.early_at.is_some_and(|e|e>=r.due_at){return Err("无效的提醒时间".into());}}
        }
        let mut note_ids=std::collections::HashSet::new();
        if self.notes.len()>200||self.notifications.len()>100_000{return Err("便签或通知数量超出上限".into());}
        for n in &mut self.notes {if !note_ids.insert(n.id)||n.body.len()>1_000_000||n.title.len()>1000||n.body.contains('\0')||n.title.contains('\0'){return Err("便签数据无效".into());}n.width=n.width.clamp(260,1600);n.height=n.height.clamp(220,1400);}
        self.next_id = self.next_id.max(self.tasks.iter().map(|t| t.id).max().unwrap_or(0).checked_add(1).ok_or("任务 ID 溢出")?);
        self.settings.width = self.settings.width.clamp(260, 1000);
        self.settings.height = self.settings.height.clamp(240, 1200);
        self.settings.opacity = self.settings.opacity.max(51);
        self.settings.draft = self.settings.draft.replace('\0', "").chars().take(10_000).collect();
        Ok(())
    }
    pub fn pending(&self) -> usize { self.tasks.iter().filter(|t| t.deleted_at.is_none() && t.completed_at.is_none()).count() }
    pub fn visible(&self, archive: bool) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.deleted_at.is_none() && t.completed_at.is_some() == archive).collect()
    }
}

pub struct Model { pub doc: Document, history: Vec<Vec<Task>> }
impl Model {
    pub fn new(mut doc: Document) -> Self {
        for t in &mut doc.tasks {
            if !t.reminder_configured { t.reminder=None; }
            t.schedule_checked=true;
        }
        Self { doc, history: vec![] }
    }
    pub fn collect_reminders(&mut self,at:u64)->bool{
        let mut fresh=vec![];
        for task in &mut self.doc.tasks{
            if task.completed_at.is_some()||task.deleted_at.is_some(){continue;}
            let Some(r)=task.reminder.as_mut()else{continue;};if !r.enabled{continue;}
            // When waking after the deadline, one due alert supersedes the stale early alert.
            if at>=r.due_at && !r.due_sent{
                fresh.push(crate::reminders::Notification{task_id:task.id,title:"待办到点提醒".into(),text:format!("{}\r\n时间：{}",task.text,crate::reminders::format_local(r.due_at)),scheduled_at:r.due_at,acknowledged:false});r.due_sent=true;r.early_sent=true;
            }else if at<r.due_at && !r.early_sent && r.early_at.is_some_and(|e|at>=e){
                fresh.push(crate::reminders::Notification{task_id:task.id,title:"待办提前提醒".into(),text:format!("{}\r\n正式时间：{}\r\n{}",task.text,crate::reminders::format_local(r.due_at),r.early_note),scheduled_at:r.early_at.unwrap(),acknowledged:false});r.early_sent=true;
            }
        }
        let changed=!fresh.is_empty();self.doc.notifications.extend(fresh);changed
    }
    fn remember(&mut self) {
        if self.history.len() >= 30 { self.history.remove(0); }
        self.history.push(self.doc.tasks.clone());
    }
    pub fn add(&mut self, text: &str) -> usize {
        let lines: Vec<String> = text.lines().map(str::trim).filter(|s| !s.is_empty())
            .map(|s| s.replace('\0', "").chars().take(10_000).collect()).collect();
        if lines.is_empty() { return 0; }
        self.remember();
        for text in &lines {
            let id = self.doc.next_id;
            self.doc.next_id += 1;
            self.doc.tasks.push(Task { id, text: text.clone(), created_at: now(), completed_at: None, deleted_at: None, reminder:None, reminder_configured:false, schedule_checked:true });
        }
        lines.len()
    }
    pub fn edit(&mut self, id: u64, text: &str) {
        let text = text.trim().replace('\0', "");
        if text.is_empty() || !self.doc.tasks.iter().any(|t| t.id == id) { return; }
        self.remember();
        if let Some(t) = self.doc.tasks.iter_mut().find(|t| t.id == id) { t.text = text.chars().take(10_000).collect(); }
    }
    pub fn toggle(&mut self, id: u64) {
        if !self.doc.tasks.iter().any(|t| t.id == id) { return; }
        self.remember();
        let t = self.doc.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.completed_at = if t.completed_at.is_some() { None } else { Some(now()) };
    }
    pub fn delete(&mut self, id: u64) {
        if !self.doc.tasks.iter().any(|t| t.id == id) { return; }
        self.remember();
        self.doc.tasks.iter_mut().find(|t| t.id == id).unwrap().deleted_at = Some(now());
    }
    pub fn restore_deleted(&mut self) -> bool {
        let id = self.doc.tasks.iter().filter(|t| t.deleted_at.is_some()).max_by_key(|t| t.deleted_at).map(|t| t.id);
        if let Some(id) = id { self.remember(); self.doc.tasks.iter_mut().find(|t| t.id == id).unwrap().deleted_at = None; true } else { false }
    }
    pub fn undo(&mut self) -> bool {
        if let Some(mut tasks) = self.history.pop() {
            // Undo task edits must not resend notifications already delivered.
            for t in &mut tasks {if let Some(r)=t.reminder.as_mut(){for n in self.doc.notifications.iter().filter(|n|n.task_id==t.id){if n.scheduled_at==r.due_at{r.due_sent=true;}if Some(n.scheduled_at)==r.early_at{r.early_sent=true;}}}}
            self.doc.tasks = tasks; true
        } else { false }
    }
    pub fn reorder(&mut self, from: u64, to: u64) {
        if from == to { return; }
        let a = self.doc.tasks.iter().position(|t| t.id == from);
        let b = self.doc.tasks.iter().position(|t| t.id == to);
        if let (Some(a), Some(b)) = (a, b) { self.remember(); let t = self.doc.tasks.remove(a); self.doc.tasks.insert(b, t); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn chinese_paste_complete_undo() {
        let mut m = Model::new(Document::default());
        assert_eq!(m.add("买咖啡\r\n\r\n整理方案 📝\n"), 2);
        m.toggle(1); assert_eq!(m.doc.pending(), 1);
        assert_eq!(m.doc.visible(true)[0].text, "买咖啡");
        m.undo(); assert_eq!(m.doc.pending(), 2);
    }
    #[test] fn delete_survives_restart_and_restore() {
        let mut m = Model::new(Document::default()); m.add("重要任务"); m.delete(1);
        let json = serde_json::to_string(&m.doc).unwrap();
        let mut m = Model::new(serde_json::from_str(&json).unwrap());
        assert_eq!(m.doc.pending(), 0); assert!(m.restore_deleted()); assert_eq!(m.doc.pending(), 1);
    }
    #[test] fn reorder_ignores_archived_without_losing_it() {
        let mut m = Model::new(Document::default()); m.add("A\nB\nC"); m.toggle(2); m.reorder(1,3);
        assert_eq!(m.doc.visible(false).iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), vec!["C","A"]);
        assert_eq!(m.doc.visible(true).len(), 1);
    }
    #[test] fn undo_never_reuses_ids() {
        let mut m = Model::new(Document::default()); m.add("A"); m.undo(); m.add("B"); assert_eq!(m.doc.tasks[0].id, 2);
    }
    #[test] fn reject_future_and_duplicate_ids() {
        let mut d = Document { version: 99, ..Default::default() }; assert!(d.validate().is_err());
        let mut m = Model::new(Document::default()); m.add("A\nB"); m.doc.tasks[1].id = 1; assert!(m.doc.validate().is_err());
    }
    #[test]fn early_due_restart_and_disabled(){
        let mut m=Model::new(Document::default());m.add("示例");m.doc.tasks[0].reminder=Some(crate::reminders::Reminder{enabled:true,due_at:1000,early_at:Some(500),early_note:"打印简历".into(),due_sent:false,early_sent:false});m.doc.tasks[0].reminder_configured=true;
        assert!(!m.collect_reminders(499));assert!(m.collect_reminders(500));assert!(!m.collect_reminders(501));
        let d=serde_json::from_str(&serde_json::to_string(&m.doc).unwrap()).unwrap();let mut m=Model::new(d);
        assert!(!m.collect_reminders(600));assert!(m.collect_reminders(1000));assert!(!m.collect_reminders(1001));assert_eq!(m.doc.notifications.len(),2);
        m.doc.tasks[0].reminder.as_mut().unwrap().due_sent=false;m.doc.tasks[0].reminder.as_mut().unwrap().enabled=false;assert!(!m.collect_reminders(2000));
    }
    #[test]fn overdue_coalesces_and_completed_suppresses(){let mut m=Model::new(Document::default());m.add("A");m.doc.tasks[0].reminder=Some(crate::reminders::Reminder{enabled:true,due_at:1000,early_at:Some(500),early_note:String::new(),due_sent:false,early_sent:false});m.doc.tasks[0].reminder_configured=true;assert!(m.collect_reminders(2000));assert_eq!(m.doc.notifications.len(),1);m.toggle(1);m.doc.tasks[0].reminder.as_mut().unwrap().due_sent=false;assert!(!m.collect_reminders(2001));}
    #[test]fn new_tasks_and_inferred_legacy_reminders_start_disabled(){let mut m=Model::new(Document::default());m.add("9月9日 19:00 vivo 宣讲会");assert!(m.doc.tasks[0].reminder.is_none());let mut d=m.doc.clone();d.tasks[0].reminder=Some(crate::reminders::Reminder{enabled:true,due_at:1000,early_at:None,early_note:String::new(),due_sent:false,early_sent:false});d.tasks[0].reminder_configured=false;assert!(Model::new(d).doc.tasks[0].reminder.is_none());}
}
