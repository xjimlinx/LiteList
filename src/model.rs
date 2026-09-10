//! Safe, platform-independent task model. Deleted items remain recoverable.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_JSON_ID: u64 = 9_007_199_254_740_991;

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub deleted_at: Option<u64>,
    #[serde(default)]
    pub reminder: Option<Reminder>,
    /// Old releases inferred reminders from text. Only explicit settings survive migration.
    #[serde(default)]
    pub reminder_configured: bool,
    #[serde(default)]
    pub schedule_checked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Reminder {
    pub enabled: bool,
    pub due_at: u64,
    pub early_at: Option<u64>,
    #[serde(default)]
    pub early_note: String,
    #[serde(default)]
    pub due_sent: bool,
    #[serde(default)]
    pub early_sent: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Notification {
    pub task_id: u64,
    pub title: String,
    pub text: String,
    pub scheduled_at: u64,
    pub acknowledged: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub visible: bool,
    pub topmost: bool,
}
impl Default for Note {
    fn default() -> Self {
        Self {
            id: 0,
            title: "桌面便签".into(),
            body: String::new(),
            x: 120,
            y: 120,
            width: 360,
            height: 360,
            visible: true,
            topmost: false,
        }
    }
}

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
    pub goal_node_shape: i8,
    pub collapsed_goal_ids: Vec<u64>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            x: -32000,
            y: -32000,
            width: 340,
            height: 440,
            collapsed: false,
            topmost: true,
            opacity: 220,
            light: false,
            draft: String::new(),
            goal_node_shape: 0,
            collapsed_goal_ids: vec![],
        }
    }
}

/// Cross-platform goal data shared with the Plasma document format.
/// -1 inherits the platform default; 0/1/2 are rounded/rectangular/pill.
fn inherited_shape() -> i8 {
    -1
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GoalNode {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub requires: Vec<u64>,
    #[serde(default = "inherited_shape")]
    pub shape: i8,
    #[serde(default = "now")]
    pub created_at: u64,
    #[serde(default)]
    pub completed_at: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Goal {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "now")]
    pub created_at: u64,
    #[serde(default)]
    pub completed_at: Option<u64>,
    #[serde(default)]
    pub nodes: Vec<GoalNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GoalProgress {
    pub completed: usize,
    pub total: usize,
    pub ratio: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GoalNodeLayout {
    pub node_id: u64,
    pub x: f32,
    pub y: f32,
    pub level: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GoalLayout {
    pub nodes: Vec<GoalNodeLayout>,
    pub width: f32,
    pub height: f32,
}

impl Goal {
    pub fn find_node(&self, id: u64) -> Option<&GoalNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn node_unlocked(&self, node_id: u64) -> bool {
        let Some(node) = self.find_node(node_id) else {
            return false;
        };
        node.requires.iter().all(|required| {
            self.find_node(*required)
                .is_some_and(|node| node.completed_at.is_some())
        })
    }

    pub fn progress(&self) -> GoalProgress {
        let total = self.nodes.len();
        let completed = self
            .nodes
            .iter()
            .filter(|node| node.completed_at.is_some())
            .count();
        GoalProgress {
            completed,
            total,
            ratio: if total == 0 {
                0.0
            } else {
                completed as f32 / total as f32
            },
        }
    }

    pub fn layout(
        &self,
        node_width: f32,
        node_height: f32,
        horizontal_gap: f32,
        vertical_gap: f32,
    ) -> GoalLayout {
        if self.nodes.is_empty() {
            return GoalLayout {
                nodes: vec![],
                width: node_width,
                height: node_height,
            };
        }
        fn level_for(goal: &Goal, id: u64, levels: &mut HashMap<u64, usize>) -> usize {
            if let Some(level) = levels.get(&id) {
                return *level;
            }
            let level = goal
                .find_node(id)
                .map(|node| {
                    node.requires
                        .iter()
                        .filter_map(|required| goal.find_node(*required))
                        .map(|required| level_for(goal, required.id, levels) + 1)
                        .max()
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            levels.insert(id, level);
            level
        }
        let mut levels = HashMap::new();
        let mut groups: Vec<Vec<u64>> = vec![];
        for node in &self.nodes {
            let level = level_for(self, node.id, &mut levels);
            if groups.len() <= level {
                groups.resize_with(level + 1, Vec::new);
            }
            groups[level].push(node.id);
        }
        let widest = groups.iter().map(Vec::len).max().unwrap_or(1);
        let width = widest as f32 * node_width + widest.saturating_sub(1) as f32 * horizontal_gap;
        let mut nodes = vec![];
        for (level, group) in groups.iter().enumerate() {
            let group_width = group.len() as f32 * node_width
                + group.len().saturating_sub(1) as f32 * horizontal_gap;
            let offset = (width - group_width) / 2.0;
            for (column, node_id) in group.iter().enumerate() {
                nodes.push(GoalNodeLayout {
                    node_id: *node_id,
                    x: offset + column as f32 * (node_width + horizontal_gap),
                    y: vertical_gap + level as f32 * (node_height + vertical_gap),
                    level,
                });
            }
        }
        GoalLayout {
            nodes,
            width,
            height: vertical_gap
                + groups.len() as f32 * node_height
                + groups.len().saturating_sub(1) as f32 * vertical_gap,
        }
    }

    fn sync_completion(&mut self, timestamp: u64) {
        let progress = self.progress();
        self.completed_at = if progress.total > 0 && progress.completed == progress.total {
            self.completed_at.or(Some(timestamp))
        } else {
            None
        };
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub next_id: u64,
    #[serde(default = "one")]
    pub next_goal_id: u64,
    #[serde(default = "one")]
    pub next_node_id: u64,
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub goals: Vec<Goal>,
    #[serde(default)]
    pub notifications: Vec<Notification>,
}
fn one() -> u64 {
    1
}
impl Default for Document {
    fn default() -> Self {
        Self {
            version: 2,
            next_id: 1,
            next_goal_id: 1,
            next_node_id: 1,
            tasks: vec![],
            settings: Settings::default(),
            notes: vec![],
            goals: vec![],
            notifications: vec![],
        }
    }
}
impl Document {
    pub fn validate(&mut self) -> Result<(), String> {
        if ![1, 2].contains(&self.version) {
            return Err(format!(
                "不支持的数据版本 {}，请使用对应版本的 LiteList",
                self.version
            ));
        }
        self.version = 2;
        if self.tasks.len() > 100_000 {
            return Err("任务数量超过安全读取上限".into());
        }
        let mut ids = std::collections::HashSet::new();
        for t in &self.tasks {
            if t.id == 0
                || t.id > MAX_JSON_ID
                || t.text.len() > 100_000
                || t.text.contains('\0')
                || !ids.insert(t.id)
            {
                return Err("任务数据包含无效内容或重复 ID".into());
            }
            if let Some(r) = &t.reminder {
                if r.due_at > 253402214400000 || r.early_at.is_some_and(|e| e >= r.due_at) {
                    return Err("无效的提醒时间".into());
                }
            }
        }
        let mut note_ids = std::collections::HashSet::new();
        if self.notes.len() > 200 || self.notifications.len() > 100_000 {
            return Err("便签或通知数量超出上限".into());
        }
        for n in &mut self.notes {
            if n.id == 0
                || n.id > MAX_JSON_ID
                || !note_ids.insert(n.id)
                || n.body.len() > 1_000_000
                || n.title.len() > 1000
                || n.body.contains('\0')
                || n.title.contains('\0')
            {
                return Err("便签数据无效".into());
            }
            n.width = n.width.clamp(260, 1600);
            n.height = n.height.clamp(220, 1400);
        }
        if self.goals.len() > 100 {
            return Err("大目标数量超过 100 个上限".into());
        }
        let mut goal_ids = HashSet::new();
        let mut node_ids = HashSet::new();
        let mut total_nodes = 0usize;
        for goal in &mut self.goals {
            goal.title = goal
                .title
                .replace('\0', "")
                .trim()
                .chars()
                .take(200)
                .collect();
            goal.description = goal
                .description
                .replace('\0', "")
                .chars()
                .take(2000)
                .collect();
            if goal.id == 0
                || goal.id > MAX_JSON_ID
                || goal.title.is_empty()
                || !goal_ids.insert(goal.id)
            {
                return Err("大目标包含无效内容或重复 ID".into());
            }
            total_nodes = total_nodes
                .checked_add(goal.nodes.len())
                .ok_or("小目标数量溢出")?;
            if total_nodes > 2000 {
                return Err("小目标数量超过 2000 个上限".into());
            }
            for node in &mut goal.nodes {
                node.title = node
                    .title
                    .replace('\0', "")
                    .trim()
                    .chars()
                    .take(200)
                    .collect();
                node.description = node
                    .description
                    .replace('\0', "")
                    .chars()
                    .take(2000)
                    .collect();
                node.requires.retain(|id| *id > 0);
                let mut requirements = HashSet::new();
                node.requires.retain(|id| requirements.insert(*id));
                if !(-1..=2).contains(&node.shape) {
                    node.shape = -1;
                }
                if node.id == 0
                    || node.id > MAX_JSON_ID
                    || node.title.is_empty()
                    || !node_ids.insert(node.id)
                {
                    return Err("小目标包含无效内容或重复 ID".into());
                }
            }
        }
        for goal in &self.goals {
            validate_goal_dependencies(goal)?;
        }
        self.next_id = self.next_id.max(
            self.tasks
                .iter()
                .map(|t| t.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("任务 ID 溢出")?,
        );
        self.next_goal_id = self.next_goal_id.max(
            self.goals
                .iter()
                .map(|goal| goal.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("大目标 ID 溢出")?,
        );
        self.next_node_id = self.next_node_id.max(
            self.goals
                .iter()
                .flat_map(|goal| &goal.nodes)
                .map(|node| node.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("小目标 ID 溢出")?,
        );
        if self.next_id > MAX_JSON_ID
            || self.next_goal_id > MAX_JSON_ID
            || self.next_node_id > MAX_JSON_ID
        {
            return Err("ID 超出跨平台 JSON 安全范围".into());
        }
        self.settings.width = self.settings.width.clamp(260, 1000);
        self.settings.height = self.settings.height.clamp(240, 1200);
        self.settings.opacity = self.settings.opacity.max(51);
        if !(0..=2).contains(&self.settings.goal_node_shape) {
            self.settings.goal_node_shape = 0;
        }
        let known_goals: HashSet<u64> = self.goals.iter().map(|goal| goal.id).collect();
        let mut collapsed = HashSet::new();
        self.settings
            .collapsed_goal_ids
            .retain(|id| known_goals.contains(id) && collapsed.insert(*id));
        self.settings.draft = self
            .settings
            .draft
            .replace('\0', "")
            .chars()
            .take(10_000)
            .collect();
        Ok(())
    }
    pub fn pending(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.deleted_at.is_none() && t.completed_at.is_none())
            .count()
    }
    pub fn visible(&self, archive: bool) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.deleted_at.is_none() && t.completed_at.is_some() == archive)
            .collect()
    }

    pub fn find_goal(&self, id: u64) -> Option<&Goal> {
        self.goals.iter().find(|goal| goal.id == id)
    }

    pub fn add_goal(&mut self, title: &str, description: &str) -> Result<u64, String> {
        if self.goals.len() >= 100 {
            return Err("大目标已达到 100 个上限".into());
        }
        let title: String = title.replace('\0', "").trim().chars().take(200).collect();
        if title.is_empty() {
            return Err("请输入大目标名称".into());
        }
        let id = self.next_goal_id;
        if id == 0 || id >= MAX_JSON_ID {
            return Err("大目标 ID 溢出".into());
        }
        self.next_goal_id = self.next_goal_id.checked_add(1).ok_or("大目标 ID 溢出")?;
        self.goals.push(Goal {
            id,
            title,
            description: description
                .replace('\0', "")
                .trim()
                .chars()
                .take(2000)
                .collect(),
            created_at: now(),
            completed_at: None,
            nodes: vec![],
        });
        Ok(id)
    }

    pub fn update_goal(
        &mut self,
        goal_id: u64,
        title: &str,
        description: &str,
    ) -> Result<(), String> {
        let title: String = title.replace('\0', "").trim().chars().take(200).collect();
        if title.is_empty() {
            return Err("请输入大目标名称".into());
        }
        let goal = self
            .goals
            .iter_mut()
            .find(|goal| goal.id == goal_id)
            .ok_or("找不到大目标")?;
        goal.title = title;
        goal.description = description
            .replace('\0', "")
            .trim()
            .chars()
            .take(2000)
            .collect();
        Ok(())
    }

    pub fn delete_goal(&mut self, goal_id: u64) -> Result<(), String> {
        let index = self
            .goals
            .iter()
            .position(|goal| goal.id == goal_id)
            .ok_or("找不到大目标")?;
        self.goals.remove(index);
        Ok(())
    }

    pub fn add_goal_node(
        &mut self,
        goal_id: u64,
        title: &str,
        description: &str,
        requires: Vec<u64>,
        shape: i8,
    ) -> Result<u64, String> {
        if self
            .goals
            .iter()
            .map(|goal| goal.nodes.len())
            .sum::<usize>()
            >= 2000
        {
            return Err("小目标已达到 2000 个上限".into());
        }
        let title: String = title.replace('\0', "").trim().chars().take(200).collect();
        if title.is_empty() {
            return Err("请输入小目标名称".into());
        }
        let goal = self
            .goals
            .iter_mut()
            .find(|goal| goal.id == goal_id)
            .ok_or("找不到大目标")?;
        let mut seen = HashSet::new();
        let requires: Vec<u64> = requires.into_iter().filter(|id| seen.insert(*id)).collect();
        if requires.iter().any(|id| goal.find_node(*id).is_none()) {
            return Err("小目标包含不存在的前置条件".into());
        }
        let id = self.next_node_id;
        if id == 0 || id >= MAX_JSON_ID {
            return Err("小目标 ID 溢出".into());
        }
        self.next_node_id = self.next_node_id.checked_add(1).ok_or("小目标 ID 溢出")?;
        goal.nodes.push(GoalNode {
            id,
            title,
            description: description
                .replace('\0', "")
                .trim()
                .chars()
                .take(2000)
                .collect(),
            requires,
            shape: if (-1..=2).contains(&shape) { shape } else { -1 },
            created_at: now(),
            completed_at: None,
        });
        goal.completed_at = None;
        Ok(id)
    }

    pub fn update_goal_node(
        &mut self,
        goal_id: u64,
        node_id: u64,
        title: &str,
        description: &str,
        shape: i8,
    ) -> Result<(), String> {
        let title: String = title.replace('\0', "").trim().chars().take(200).collect();
        if title.is_empty() {
            return Err("请输入小目标名称".into());
        }
        let node = self
            .goals
            .iter_mut()
            .find(|goal| goal.id == goal_id)
            .and_then(|goal| goal.nodes.iter_mut().find(|node| node.id == node_id))
            .ok_or("找不到小目标")?;
        node.title = title;
        node.description = description
            .replace('\0', "")
            .trim()
            .chars()
            .take(2000)
            .collect();
        node.shape = if (-1..=2).contains(&shape) { shape } else { -1 };
        Ok(())
    }

    pub fn delete_goal_node(&mut self, goal_id: u64, node_id: u64) -> Result<(), String> {
        let goal = self
            .goals
            .iter_mut()
            .find(|goal| goal.id == goal_id)
            .ok_or("找不到大目标")?;
        if goal
            .nodes
            .iter()
            .any(|node| node.requires.contains(&node_id))
        {
            return Err("该节点仍是其他小目标的前置条件，无法删除".into());
        }
        let index = goal
            .nodes
            .iter()
            .position(|node| node.id == node_id)
            .ok_or("找不到小目标")?;
        goal.nodes.remove(index);
        goal.sync_completion(now());
        Ok(())
    }

    pub fn toggle_goal_node(
        &mut self,
        goal_id: u64,
        node_id: u64,
        timestamp: u64,
    ) -> Result<bool, String> {
        let goal = self
            .goals
            .iter_mut()
            .find(|goal| goal.id == goal_id)
            .ok_or("找不到大目标")?;
        let node = goal.find_node(node_id).ok_or("找不到小目标")?;
        let completing = node.completed_at.is_none();
        if completing && !goal.node_unlocked(node_id) {
            return Err("请先完成前置小目标".into());
        }
        if !completing
            && goal.nodes.iter().any(|dependent| {
                dependent.completed_at.is_some() && dependent.requires.contains(&node_id)
            })
        {
            return Err("已有完成节点依赖它，暂时不能撤回".into());
        }
        goal.nodes
            .iter_mut()
            .find(|node| node.id == node_id)
            .unwrap()
            .completed_at = if completing { Some(timestamp) } else { None };
        goal.sync_completion(timestamp);
        Ok(completing)
    }
}

fn validate_goal_dependencies(goal: &Goal) -> Result<(), String> {
    let local_ids: HashSet<u64> = goal.nodes.iter().map(|node| node.id).collect();
    for node in &goal.nodes {
        if node
            .requires
            .iter()
            .any(|required| *required == node.id || !local_ids.contains(required))
        {
            return Err("小目标包含不存在或指向自身的前置条件".into());
        }
    }
    fn visit(
        goal: &Goal,
        node_id: u64,
        visiting: &mut HashSet<u64>,
        visited: &mut HashSet<u64>,
    ) -> Result<(), String> {
        if visiting.contains(&node_id) {
            return Err("小目标前置条件形成了循环".into());
        }
        if visited.contains(&node_id) {
            return Ok(());
        }
        visiting.insert(node_id);
        for required in &goal.find_node(node_id).ok_or("找不到小目标")?.requires {
            visit(goal, *required, visiting, visited)?;
        }
        visiting.remove(&node_id);
        visited.insert(node_id);
        Ok(())
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for node in &goal.nodes {
        visit(goal, node.id, &mut visiting, &mut visited)?;
    }
    Ok(())
}

pub struct Model {
    pub doc: Document,
    history: Vec<Vec<Task>>,
}
impl Model {
    pub fn new(mut doc: Document) -> Self {
        for t in &mut doc.tasks {
            if !t.reminder_configured {
                t.reminder = None;
            }
            t.schedule_checked = true;
        }
        Self {
            doc,
            history: vec![],
        }
    }
    pub fn collect_reminders(&mut self, at: u64) -> bool {
        let mut fresh = vec![];
        for task in &mut self.doc.tasks {
            if task.completed_at.is_some() || task.deleted_at.is_some() {
                continue;
            }
            let Some(r) = task.reminder.as_mut() else {
                continue;
            };
            if !r.enabled {
                continue;
            }
            // When waking after the deadline, one due alert supersedes the stale early alert.
            if at >= r.due_at && !r.due_sent {
                fresh.push(Notification {
                    task_id: task.id,
                    title: "待办到点提醒".into(),
                    text: format!(
                        "{}\r\n时间：{}",
                        task.text,
                        format_notification_time(r.due_at)
                    ),
                    scheduled_at: r.due_at,
                    acknowledged: false,
                });
                r.due_sent = true;
                r.early_sent = true;
            } else if at < r.due_at && !r.early_sent && r.early_at.is_some_and(|e| at >= e) {
                fresh.push(Notification {
                    task_id: task.id,
                    title: "待办提前提醒".into(),
                    text: format!(
                        "{}\r\n正式时间：{}\r\n{}",
                        task.text,
                        format_notification_time(r.due_at),
                        r.early_note
                    ),
                    scheduled_at: r.early_at.unwrap(),
                    acknowledged: false,
                });
                r.early_sent = true;
            }
        }
        let changed = !fresh.is_empty();
        self.doc.notifications.extend(fresh);
        changed
    }
    fn remember(&mut self) {
        if self.history.len() >= 30 {
            self.history.remove(0);
        }
        self.history.push(self.doc.tasks.clone());
    }
    pub fn add(&mut self, text: &str) -> usize {
        if self.doc.next_id == 0 || self.doc.next_id >= MAX_JSON_ID {
            return 0;
        }
        let lines: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.replace('\0', "").chars().take(10_000).collect())
            .collect();
        if lines.is_empty() {
            return 0;
        }
        self.remember();
        for text in &lines {
            let id = self.doc.next_id;
            self.doc.next_id += 1;
            self.doc.tasks.push(Task {
                id,
                text: text.clone(),
                created_at: now(),
                completed_at: None,
                deleted_at: None,
                reminder: None,
                reminder_configured: false,
                schedule_checked: true,
            });
        }
        lines.len()
    }
    pub fn edit(&mut self, id: u64, text: &str) {
        let text = text.trim().replace('\0', "");
        if text.is_empty() || !self.doc.tasks.iter().any(|t| t.id == id) {
            return;
        }
        self.remember();
        if let Some(t) = self.doc.tasks.iter_mut().find(|t| t.id == id) {
            t.text = text.chars().take(10_000).collect();
        }
    }
    pub fn toggle(&mut self, id: u64) {
        if !self.doc.tasks.iter().any(|t| t.id == id) {
            return;
        }
        self.remember();
        let t = self.doc.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.completed_at = if t.completed_at.is_some() {
            None
        } else {
            Some(now())
        };
    }
    pub fn delete(&mut self, id: u64) {
        if !self.doc.tasks.iter().any(|t| t.id == id) {
            return;
        }
        self.remember();
        self.doc
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .deleted_at = Some(now());
    }
    pub fn restore_deleted(&mut self) -> bool {
        let id = self
            .doc
            .tasks
            .iter()
            .filter(|t| t.deleted_at.is_some())
            .max_by_key(|t| t.deleted_at)
            .map(|t| t.id);
        if let Some(id) = id {
            self.remember();
            self.doc
                .tasks
                .iter_mut()
                .find(|t| t.id == id)
                .unwrap()
                .deleted_at = None;
            true
        } else {
            false
        }
    }
    pub fn undo(&mut self) -> bool {
        if let Some(mut tasks) = self.history.pop() {
            // Undo task edits must not resend notifications already delivered.
            for t in &mut tasks {
                if let Some(r) = t.reminder.as_mut() {
                    for n in self.doc.notifications.iter().filter(|n| n.task_id == t.id) {
                        if n.scheduled_at == r.due_at {
                            r.due_sent = true;
                        }
                        if Some(n.scheduled_at) == r.early_at {
                            r.early_sent = true;
                        }
                    }
                }
            }
            self.doc.tasks = tasks;
            true
        } else {
            false
        }
    }
    pub fn reorder(&mut self, from: u64, to: u64) {
        if from == to {
            return;
        }
        let a = self.doc.tasks.iter().position(|t| t.id == from);
        let b = self.doc.tasks.iter().position(|t| t.id == to);
        if let (Some(a), Some(b)) = (a, b) {
            self.remember();
            let t = self.doc.tasks.remove(a);
            self.doc.tasks.insert(b, t);
        }
    }
}

#[cfg(windows)]
fn format_notification_time(timestamp: u64) -> String {
    crate::reminders::format_local(timestamp)
}

// Domain tests run on Linux without linking Win32 timezone APIs. The timestamp
// formatting itself remains covered by reminders.rs on Windows.
#[cfg(not(windows))]
fn format_notification_time(timestamp: u64) -> String {
    timestamp.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chinese_paste_complete_undo() {
        let mut m = Model::new(Document::default());
        assert_eq!(m.add("买咖啡\r\n\r\n整理方案 📝\n"), 2);
        m.toggle(1);
        assert_eq!(m.doc.pending(), 1);
        assert_eq!(m.doc.visible(true)[0].text, "买咖啡");
        m.undo();
        assert_eq!(m.doc.pending(), 2);
    }
    #[test]
    fn delete_survives_restart_and_restore() {
        let mut m = Model::new(Document::default());
        m.add("重要任务");
        m.delete(1);
        let json = serde_json::to_string(&m.doc).unwrap();
        let mut m = Model::new(serde_json::from_str(&json).unwrap());
        assert_eq!(m.doc.pending(), 0);
        assert!(m.restore_deleted());
        assert_eq!(m.doc.pending(), 1);
    }
    #[test]
    fn reorder_ignores_archived_without_losing_it() {
        let mut m = Model::new(Document::default());
        m.add("A\nB\nC");
        m.toggle(2);
        m.reorder(1, 3);
        assert_eq!(
            m.doc
                .visible(false)
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>(),
            vec!["C", "A"]
        );
        assert_eq!(m.doc.visible(true).len(), 1);
    }
    #[test]
    fn undo_never_reuses_ids() {
        let mut m = Model::new(Document::default());
        m.add("A");
        m.undo();
        m.add("B");
        assert_eq!(m.doc.tasks[0].id, 2);
    }
    #[test]
    fn reject_future_and_duplicate_ids() {
        let mut d = Document {
            version: 99,
            ..Default::default()
        };
        assert!(d.validate().is_err());
        let mut m = Model::new(Document::default());
        m.add("A\nB");
        m.doc.tasks[1].id = 1;
        assert!(m.doc.validate().is_err());
    }
    #[test]
    fn early_due_restart_and_disabled() {
        let mut m = Model::new(Document::default());
        m.add("示例");
        m.doc.tasks[0].reminder = Some(Reminder {
            enabled: true,
            due_at: 1000,
            early_at: Some(500),
            early_note: "打印简历".into(),
            due_sent: false,
            early_sent: false,
        });
        m.doc.tasks[0].reminder_configured = true;
        assert!(!m.collect_reminders(499));
        assert!(m.collect_reminders(500));
        assert!(!m.collect_reminders(501));
        let d = serde_json::from_str(&serde_json::to_string(&m.doc).unwrap()).unwrap();
        let mut m = Model::new(d);
        assert!(!m.collect_reminders(600));
        assert!(m.collect_reminders(1000));
        assert!(!m.collect_reminders(1001));
        assert_eq!(m.doc.notifications.len(), 2);
        m.doc.tasks[0].reminder.as_mut().unwrap().due_sent = false;
        m.doc.tasks[0].reminder.as_mut().unwrap().enabled = false;
        assert!(!m.collect_reminders(2000));
    }
    #[test]
    fn overdue_coalesces_and_completed_suppresses() {
        let mut m = Model::new(Document::default());
        m.add("A");
        m.doc.tasks[0].reminder = Some(Reminder {
            enabled: true,
            due_at: 1000,
            early_at: Some(500),
            early_note: String::new(),
            due_sent: false,
            early_sent: false,
        });
        m.doc.tasks[0].reminder_configured = true;
        assert!(m.collect_reminders(2000));
        assert_eq!(m.doc.notifications.len(), 1);
        m.toggle(1);
        m.doc.tasks[0].reminder.as_mut().unwrap().due_sent = false;
        assert!(!m.collect_reminders(2001));
    }
    #[test]
    fn new_tasks_and_inferred_legacy_reminders_start_disabled() {
        let mut m = Model::new(Document::default());
        m.add("9月9日 19:00 vivo 宣讲会");
        assert!(m.doc.tasks[0].reminder.is_none());
        let mut d = m.doc.clone();
        d.tasks[0].reminder = Some(Reminder {
            enabled: true,
            due_at: 1000,
            early_at: None,
            early_note: String::new(),
            due_sent: false,
            early_sent: false,
        });
        d.tasks[0].reminder_configured = false;
        assert!(Model::new(d).doc.tasks[0].reminder.is_none());
    }

    #[test]
    fn shared_goal_fixture_round_trips_without_loss() {
        let source = include_str!("../shared/fixtures/goals-v2.json");
        let mut document: Document = serde_json::from_str(source).unwrap();
        document.validate().unwrap();
        assert_eq!(document.goals.len(), 2);
        assert_eq!(document.goals[0].nodes[1].shape, 2);
        assert_eq!(document.goals[0].nodes[2].requires, vec![1, 2]);
        let encoded = serde_json::to_string(&document).unwrap();
        let decoded: Document = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.goals, document.goals);
        assert_eq!(decoded.next_goal_id, 3);
        assert_eq!(decoded.next_node_id, 4);
    }

    #[test]
    fn goal_rules_match_plasma_contract() {
        let mut document = Document::default();
        let goal_id = document
            .add_goal("发布 Plasma 版本", "完成首个公开版本")
            .unwrap();
        let ui = document
            .add_goal_node(goal_id, "完成界面", "", vec![], -1)
            .unwrap();
        let test = document
            .add_goal_node(goal_id, "完成测试", "", vec![ui], 2)
            .unwrap();
        let publish = document
            .add_goal_node(goal_id, "发布", "", vec![ui, test], 1)
            .unwrap();
        let goal = document.find_goal(goal_id).unwrap();
        assert!(goal.node_unlocked(ui));
        assert!(!goal.node_unlocked(test));
        let layout = goal.layout(150.0, 80.0, 24.0, 48.0);
        assert_eq!(
            layout
                .nodes
                .iter()
                .find(|node| node.node_id == test)
                .unwrap()
                .level,
            1
        );
        assert_eq!(
            layout
                .nodes
                .iter()
                .find(|node| node.node_id == publish)
                .unwrap()
                .level,
            2
        );
        assert!(document.toggle_goal_node(goal_id, test, 100).is_err());
        assert_eq!(document.toggle_goal_node(goal_id, ui, 100).unwrap(), true);
        assert_eq!(document.toggle_goal_node(goal_id, test, 200).unwrap(), true);
        assert!(document.toggle_goal_node(goal_id, ui, 300).is_err());
        assert_eq!(
            document.toggle_goal_node(goal_id, publish, 300).unwrap(),
            true
        );
        assert_eq!(document.find_goal(goal_id).unwrap().completed_at, Some(300));
        document
            .update_goal(goal_id, "统一功能", "保持平台原生界面")
            .unwrap();
        document
            .update_goal_node(goal_id, publish, "Windows 界面", "GDI+", 1)
            .unwrap();
        assert_eq!(document.find_goal(goal_id).unwrap().title, "统一功能");
        assert_eq!(
            document
                .find_goal(goal_id)
                .unwrap()
                .find_node(publish)
                .unwrap()
                .shape,
            1
        );
        assert!(document.delete_goal_node(goal_id, ui).is_err());
        document.delete_goal_node(goal_id, publish).unwrap();
        document.delete_goal_node(goal_id, test).unwrap();
        document.delete_goal_node(goal_id, ui).unwrap();
        document.delete_goal(goal_id).unwrap();
        assert!(document.goals.is_empty());
    }

    #[test]
    fn rejects_invalid_goal_dependencies() {
        let source = include_str!("../shared/fixtures/goals-v2.json");
        let mut document: Document = serde_json::from_str(source).unwrap();
        document.goals[0].nodes[0].requires = vec![3];
        assert!(document.validate().is_err());
    }
}
