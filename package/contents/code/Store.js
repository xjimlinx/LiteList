.pragma library

function now() {
    return Date.now()
}

function defaultDocument() {
    return {
        version: 2,
        next_id: 1,
        tasks: [],
        settings: {},
        notes: [],
        notifications: []
    }
}

function clone(value) {
    return JSON.parse(JSON.stringify(value))
}

function stringValue(value, limit) {
    if (typeof value !== "string")
        return ""
    return value.replace(/\u0000/g, "").slice(0, limit)
}

function normalizeDocument(value) {
    if (!value || typeof value !== "object")
        throw new Error("根节点不是对象")
    if (value.version !== 1 && value.version !== 2)
        throw new Error("不支持的数据版本 " + value.version)
    if (!Array.isArray(value.tasks) || value.tasks.length > 100000)
        throw new Error("任务数据无效或超过上限")

    const result = defaultDocument()
    const ids = {}
    let largestId = 0
    for (let index = 0; index < value.tasks.length; ++index) {
        const source = value.tasks[index]
        const id = Number(source.id)
        const text = stringValue(source.text, 10000).trim()
        if (!Number.isSafeInteger(id) || id < 1 || ids[id] || text.length === 0)
            throw new Error("任务包含无效内容或重复 ID")
        ids[id] = true
        largestId = Math.max(largestId, id)
        let reminder = null
        if (source.reminder_configured && source.reminder) {
            const due = Number(source.reminder.due_at)
            const early = source.reminder.early_at === null || source.reminder.early_at === undefined
                ? null : Number(source.reminder.early_at)
            if (!Number.isFinite(due) || due < 0 || (early !== null && (!Number.isFinite(early) || early >= due)))
                throw new Error("提醒时间无效")
            reminder = {
                enabled: Boolean(source.reminder.enabled),
                due_at: due,
                early_at: early,
                early_note: stringValue(source.reminder.early_note, 1000),
                due_sent: Boolean(source.reminder.due_sent),
                early_sent: Boolean(source.reminder.early_sent)
            }
        }
        result.tasks.push({
            id: id,
            text: text,
            created_at: Number(source.created_at) || now(),
            completed_at: source.completed_at === null || source.completed_at === undefined ? null : Number(source.completed_at),
            deleted_at: source.deleted_at === null || source.deleted_at === undefined ? null : Number(source.deleted_at),
            reminder: reminder,
            reminder_configured: reminder !== null,
            schedule_checked: true
        })
    }

    if (Array.isArray(value.notes)) {
        const noteIds = {}
        for (let index = 0; index < Math.min(value.notes.length, 200); ++index) {
            const source = value.notes[index]
            const id = Number(source.id)
            if (!Number.isSafeInteger(id) || id < 1 || noteIds[id])
                throw new Error("便签包含无效或重复 ID")
            noteIds[id] = true
            result.notes.push({
                id: id,
                title: stringValue(source.title, 150) || "桌面便签",
                body: stringValue(source.body, 200000),
                visible: source.visible !== false,
                topmost: Boolean(source.topmost),
                x: Number(source.x) || 0,
                y: Number(source.y) || 0,
                width: Number(source.width) || 360,
                height: Number(source.height) || 360
            })
        }
    }

    if (Array.isArray(value.notifications)) {
        for (let index = 0; index < Math.min(value.notifications.length, 100000); ++index) {
            const source = value.notifications[index]
            result.notifications.push({
                task_id: Number(source.task_id) || 0,
                title: stringValue(source.title, 200),
                text: stringValue(source.text, 12000),
                scheduled_at: Number(source.scheduled_at) || 0,
                acknowledged: Boolean(source.acknowledged)
            })
        }
    }

    result.settings = value.settings && typeof value.settings === "object" ? value.settings : {}
    result.next_id = Math.max(Number(value.next_id) || 1, largestId + 1)
    return result
}

function load(text) {
    try {
        return { document: normalizeDocument(JSON.parse(text)), error: "" }
    } catch (error) {
        return { document: defaultDocument(), error: String(error) }
    }
}

function visibleTasks(document, archive) {
    return document.tasks.filter(function(task) {
        return task.deleted_at === null && (task.completed_at !== null) === archive
    })
}

function pendingCount(document) {
    return visibleTasks(document, false).length
}

function addTasks(document, input) {
    const lines = input.split(/\r?\n/).map(function(line) {
        return stringValue(line, 10000).trim()
    }).filter(function(line) { return line.length > 0 })
    for (let index = 0; index < lines.length; ++index) {
        document.tasks.push({
            id: document.next_id++,
            text: lines[index],
            created_at: now(),
            completed_at: null,
            deleted_at: null,
            reminder: null,
            reminder_configured: false,
            schedule_checked: true
        })
    }
    return lines.length
}

function findTask(document, id) {
    for (let index = 0; index < document.tasks.length; ++index) {
        if (document.tasks[index].id === id)
            return document.tasks[index]
    }
    return null
}

function pad(value) {
    return value < 10 ? "0" + value : String(value)
}

function formatLocal(milliseconds) {
    const date = new Date(milliseconds)
    if (!Number.isFinite(date.getTime()))
        return ""
    return date.getFullYear() + "-" + pad(date.getMonth() + 1) + "-" + pad(date.getDate())
        + " " + pad(date.getHours()) + ":" + pad(date.getMinutes())
}

function parseLocal(value) {
    const match = /^\s*(\d{4})-(\d{1,2})-(\d{1,2})\s+(\d{1,2}):(\d{2})\s*$/.exec(value)
    if (!match)
        throw new Error("请输入 YYYY-MM-DD HH:MM 格式的本地时间")
    const year = Number(match[1])
    const month = Number(match[2])
    const day = Number(match[3])
    const hour = Number(match[4])
    const minute = Number(match[5])
    const date = new Date(year, month - 1, day, hour, minute, 0, 0)
    if (year < 1970 || year > 9999 || date.getFullYear() !== year || date.getMonth() !== month - 1
            || date.getDate() !== day || date.getHours() !== hour || date.getMinutes() !== minute)
        throw new Error("日期或时间无效")
    return date.getTime()
}

function collectReminders(document, timestamp) {
    const fresh = []
    for (let index = 0; index < document.tasks.length; ++index) {
        const task = document.tasks[index]
        const reminder = task.reminder
        if (task.completed_at !== null || task.deleted_at !== null || !reminder || !reminder.enabled)
            continue
        let item = null
        if (timestamp >= reminder.due_at && !reminder.due_sent) {
            item = {
                task_id: task.id,
                title: "待办到点提醒",
                text: task.text + "\n时间：" + formatLocal(reminder.due_at),
                scheduled_at: reminder.due_at,
                acknowledged: false
            }
            reminder.due_sent = true
            reminder.early_sent = true
        } else if (timestamp < reminder.due_at && reminder.early_at !== null
                   && timestamp >= reminder.early_at && !reminder.early_sent) {
            item = {
                task_id: task.id,
                title: "待办提前提醒",
                text: task.text + "\n正式时间：" + formatLocal(reminder.due_at)
                    + (reminder.early_note ? "\n" + reminder.early_note : ""),
                scheduled_at: reminder.early_at,
                acknowledged: false
            }
            reminder.early_sent = true
        }
        if (item) {
            document.notifications.push(item)
            fresh.push(item)
        }
    }
    return fresh
}
