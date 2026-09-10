#!/usr/bin/env node

const fs = require("fs")
const path = require("path")
const vm = require("vm")

const sourcePath = path.join(__dirname, "..", "package", "contents", "code", "Store.js")
const source = fs.readFileSync(sourcePath, "utf8").replace(/^\.pragma library\s*/, "")
const Store = { console }
vm.createContext(Store)
vm.runInContext(source, Store, { filename: sourcePath })

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

console.log("Store.js: all model checks passed")
