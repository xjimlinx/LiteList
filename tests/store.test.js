#!/usr/bin/env node

const fs = require("fs")
const path = require("path")
const vm = require("vm")

const sourcePath = path.join(__dirname, "..", "package", "contents", "code", "Store.js")
const source = fs.readFileSync(sourcePath, "utf8").replace(/^\.pragma library\s*/, "")
const Store = { console }
vm.createContext(Store)
vm.runInContext(source, Store, { filename: sourcePath })
const sharedGoalFixture = fs.readFileSync(
    path.join(__dirname, "..", "shared", "fixtures", "goals-v2.json"), "utf8")

function check(condition, message) {
    if (!condition)
        throw new Error(message)
}

let document = Store.defaultDocument()
check(Store.addTasks(document, "买咖啡\n\n整理方案") === 2, "multi-line add failed")
check(Store.pendingCount(document) === 2, "pending count failed")
document.tasks[0].completed_at = Store.now()
check(Store.visibleTasks(document, true)[0].text === "买咖啡", "archive failed")

let loaded = Store.load(JSON.stringify(document))
check(loaded.error === "", "round trip was rejected")
check(loaded.document.next_id === 3, "next ID was not preserved")

document = Store.defaultDocument()
Store.addTasks(document, "提交方案")
document.tasks[0].reminder_configured = true
document.tasks[0].reminder = {
    enabled: true,
    due_at: 1000,
    early_at: 500,
    early_note: "打印附件",
    due_sent: false,
    early_sent: false
}
let fresh = Store.collectReminders(document, 2000)
check(fresh.length === 1 && fresh[0].title === "待办到点提醒", "overdue reminder was not coalesced")
check(Store.collectReminders(document, 2001).length === 0, "reminder was delivered twice")
check(Number.isFinite(Store.parseLocal("2026-09-09 19:00")), "valid local time was rejected")

let invalidDateRejected = false
try {
    Store.parseLocal("2026-02-30 19:00")
} catch (_) {
    invalidDateRejected = true
}
check(invalidDateRejected, "invalid calendar date was accepted")

document = Store.defaultDocument()
Store.addTasks(document, "A\nB")
document.tasks[1].id = document.tasks[0].id
check(Store.load(JSON.stringify(document)).error.length > 0, "duplicate IDs were accepted")

document = Store.defaultDocument()
const goal = {
    id: document.next_goal_id++,
    title: "发布 Plasma 版本",
    description: "完成首个公开版本",
    created_at: Store.now(),
    completed_at: null,
    nodes: [
        { id: document.next_node_id++, title: "完成界面", description: "", requires: [], created_at: Store.now(), completed_at: null },
        { id: document.next_node_id++, title: "完成测试", description: "", requires: [1], created_at: Store.now(), completed_at: null },
        { id: document.next_node_id++, title: "发布", description: "", requires: [1, 2], created_at: Store.now(), completed_at: null }
    ]
}
document.goals.push(goal)
goal.nodes[1].shape = 2
document.goals.push({
    id: document.next_goal_id++,
    title: "准备下一版本",
    description: "与第一个大目标并行推进",
    created_at: Store.now(),
    completed_at: null,
    nodes: []
})
const multipleGoals = Store.load(JSON.stringify(document))
check(multipleGoals.error === "", "multiple goals were rejected")
check(multipleGoals.document.goals.length === 2, "multiple goals were not preserved")
check(multipleGoals.document.goals[0].nodes[0].shape === -1,
      "legacy nodes should inherit the global shape")
check(multipleGoals.document.goals[0].nodes[1].shape === 2,
      "per-node shape was not preserved")
check(Store.nodeUnlocked(goal, goal.nodes[0]), "root goal node should be unlocked")
check(!Store.nodeUnlocked(goal, goal.nodes[1]), "dependent goal node unlocked too early")
goal.nodes[0].completed_at = Store.now()
check(Store.nodeUnlocked(goal, goal.nodes[1]), "dependent goal node stayed locked")
check(Store.goalProgress(goal).completed === 1, "goal progress is incorrect")
const layout = Store.goalLayout(goal, 150, 80, 24, 48)
check(layout.nodes.length === 3, "goal layout lost nodes")
check(layout.nodes[1].level === 1 && layout.nodes[2].level === 2, "goal prerequisite levels are incorrect")

const cyclic = Store.clone(document)
cyclic.goals[0].nodes[0].requires = [3]
check(Store.load(JSON.stringify(cyclic)).error.length > 0, "cyclic prerequisites were accepted")

const sharedLoaded = Store.load(sharedGoalFixture)
check(sharedLoaded.error === "", "shared cross-platform fixture was rejected")
check(sharedLoaded.document.goals.length === 2, "shared fixture lost a large goal")
check(sharedLoaded.document.goals[0].nodes[1].shape === 2,
      "shared fixture lost its per-node shape")
check(sharedLoaded.document.goals[0].nodes[2].requires.join(",") === "1,2",
      "shared fixture lost prerequisite order")
const sharedLayout = Store.goalLayout(sharedLoaded.document.goals[0], 150, 80, 24, 48)
check(sharedLayout.nodes.find(item => item.node.id === 2).level === 1,
      "shared fixture produced a different dependency level")

document = Store.defaultDocument()
const managedGoalId = Store.addGoal(document, "  统一版本  ", " 跨平台规则 ", 10)
const rootNodeId = Store.addGoalNode(document, managedGoalId, "模型", "", [], -1, 20)
const childNodeId = Store.addGoalNode(document, managedGoalId, "界面", "", [rootNodeId], 2, 30)
check(document.goals[0].title === "统一版本", "goal title was not normalized")
check(!Store.nodeUnlocked(document.goals[0], document.goals[0].nodes[1]),
      "managed dependent node unlocked too early")
let mutationRejected = false
try {
    Store.toggleGoalNode(document, managedGoalId, childNodeId, 40)
} catch (_) {
    mutationRejected = true
}
check(mutationRejected, "managed mutation bypassed prerequisites")
check(Store.toggleGoalNode(document, managedGoalId, rootNodeId, 50),
      "root node was not completed")
check(Store.toggleGoalNode(document, managedGoalId, childNodeId, 60),
      "child node was not completed")
check(document.goals[0].completed_at === 60, "large goal was not completed automatically")
mutationRejected = false
try {
    Store.toggleGoalNode(document, managedGoalId, rootNodeId, 70)
} catch (_) {
    mutationRejected = true
}
check(mutationRejected, "completed prerequisite was withdrawn underneath a child")
Store.updateGoal(document, managedGoalId, "统一功能", "保持平台原生界面")
Store.updateGoalNode(document, managedGoalId, childNodeId, "Windows 界面", "GDI+", 1)
check(document.goals[0].nodes[1].shape === 1, "node update lost its shape")
mutationRejected = false
try {
    Store.deleteGoalNode(document, managedGoalId, rootNodeId, 80)
} catch (_) {
    mutationRejected = true
}
check(mutationRejected, "a required node was deleted")
Store.deleteGoalNode(document, managedGoalId, childNodeId, 80)
Store.deleteGoalNode(document, managedGoalId, rootNodeId, 80)
Store.deleteGoal(document, managedGoalId)
check(document.goals.length === 0, "goal deletion failed")

console.log("Store.js: all model checks passed")
