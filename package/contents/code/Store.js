.pragma library

function now() {
    return Date.now()
}

function defaultDocument() {
    return {
        version: 2,
        next_id: 1,
        next_goal_id: 1,
        next_node_id: 1,
        tasks: [],
        settings: {},
        notes: [],
        goals: [],
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

    const goalIds = {}
    const nodeIds = {}
    let largestGoalId = 0
    let largestNodeId = 0
    let totalNodes = 0
    if (Array.isArray(value.goals)) {
        if (value.goals.length > 100)
            throw new Error("大目标数量超过 100 个上限")
        for (let goalIndex = 0; goalIndex < value.goals.length; ++goalIndex) {
            const sourceGoal = value.goals[goalIndex]
            const goalId = Number(sourceGoal.id)
            const title = stringValue(sourceGoal.title, 200).trim()
            if (!Number.isSafeInteger(goalId) || goalId < 1 || goalIds[goalId] || title.length === 0)
                throw new Error("大目标包含无效内容或重复 ID")
            goalIds[goalId] = true
            largestGoalId = Math.max(largestGoalId, goalId)
            const goal = {
                id: goalId,
                title: title,
                description: stringValue(sourceGoal.description, 2000),
                created_at: Number(sourceGoal.created_at) || now(),
                completed_at: sourceGoal.completed_at === null || sourceGoal.completed_at === undefined
                              ? null : Number(sourceGoal.completed_at),
                terminated_at: sourceGoal.terminated_at === null || sourceGoal.terminated_at === undefined
                               ? null : Number(sourceGoal.terminated_at),
                nodes: []
            }
            if (goal.terminated_at !== null)
                goal.completed_at = null
            if (!Array.isArray(sourceGoal.nodes))
                sourceGoal.nodes = []
            totalNodes += sourceGoal.nodes.length
            if (totalNodes > 2000)
                throw new Error("小目标数量超过 2000 个上限")
            for (let nodeIndex = 0; nodeIndex < sourceGoal.nodes.length; ++nodeIndex) {
                const sourceNode = sourceGoal.nodes[nodeIndex]
                const nodeId = Number(sourceNode.id)
                const nodeTitle = stringValue(sourceNode.title, 200).trim()
                if (!Number.isSafeInteger(nodeId) || nodeId < 1 || nodeIds[nodeId] || nodeTitle.length === 0)
                    throw new Error("小目标包含无效内容或重复 ID")
                nodeIds[nodeId] = true
                largestNodeId = Math.max(largestNodeId, nodeId)
                const requires = Array.isArray(sourceNode.requires)
                    ? sourceNode.requires.map(Number).filter(function(id, index, all) {
                        return Number.isSafeInteger(id) && id > 0 && all.indexOf(id) === index
                    }) : []
                const sourceShape = Number(sourceNode.shape)
                const shape = Number.isSafeInteger(sourceShape) && sourceShape >= 0 && sourceShape <= 2
                    ? sourceShape : -1
                goal.nodes.push({
                    id: nodeId,
                    title: nodeTitle,
                    description: stringValue(sourceNode.description, 2000),
                    requires: requires,
                    shape: shape,
                    created_at: Number(sourceNode.created_at) || now(),
                    completed_at: sourceNode.completed_at === null || sourceNode.completed_at === undefined
                                  ? null : Number(sourceNode.completed_at)
                })
            }
            result.goals.push(goal)
        }
    }

    for (let goalIndex = 0; goalIndex < result.goals.length; ++goalIndex) {
        const goal = result.goals[goalIndex]
        const localIds = {}
        for (let nodeIndex = 0; nodeIndex < goal.nodes.length; ++nodeIndex)
            localIds[goal.nodes[nodeIndex].id] = true
        for (let nodeIndex = 0; nodeIndex < goal.nodes.length; ++nodeIndex) {
            const node = goal.nodes[nodeIndex]
            for (let requirementIndex = 0; requirementIndex < node.requires.length; ++requirementIndex) {
                const requirement = node.requires[requirementIndex]
                if (!localIds[requirement] || requirement === node.id)
                    throw new Error("小目标包含不存在或指向自身的前置条件")
            }
        }
        const visiting = {}
        const visited = {}
        function visit(nodeId) {
            if (visiting[nodeId])
                throw new Error("小目标前置条件形成了循环")
            if (visited[nodeId])
                return
            visiting[nodeId] = true
            const node = findGoalNode(goal, nodeId)
            for (let index = 0; index < node.requires.length; ++index)
                visit(node.requires[index])
            delete visiting[nodeId]
            visited[nodeId] = true
        }
        for (let nodeIndex = 0; nodeIndex < goal.nodes.length; ++nodeIndex)
            visit(goal.nodes[nodeIndex].id)
    }

    result.settings = value.settings && typeof value.settings === "object" ? value.settings : {}
    function nextCounter(value, largest) {
        let candidate = Number(value)
        if (!Number.isSafeInteger(candidate) || candidate < 1)
            candidate = 1
        candidate = Math.max(candidate, largest + 1)
        if (!Number.isSafeInteger(candidate))
            throw new Error("ID 超出跨平台 JSON 安全范围")
        return candidate
    }
    result.next_id = nextCounter(value.next_id, largestId)
    result.next_goal_id = nextCounter(value.next_goal_id, largestGoalId)
    result.next_node_id = nextCounter(value.next_node_id, largestNodeId)
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

function findGoal(document, id) {
    for (let index = 0; index < document.goals.length; ++index) {
        if (document.goals[index].id === id)
            return document.goals[index]
    }
    return null
}

function findGoalNode(goal, id) {
    if (!goal)
        return null
    for (let index = 0; index < goal.nodes.length; ++index) {
        if (goal.nodes[index].id === id)
            return goal.nodes[index]
    }
    return null
}

function goalTerminated(goal) {
    return Boolean(goal && goal.terminated_at !== null && goal.terminated_at !== undefined)
}

function findNote(document, id) {
    for (let index = 0; index < document.notes.length; ++index) {
        if (document.notes[index].id === id)
            return document.notes[index]
    }
    return null
}

function addNote(document) {
    if (document.notes.length >= 200)
        throw new Error("便签已达到 200 张上限")
    let largest = 0
    for (let index = 0; index < document.notes.length; ++index)
        largest = Math.max(largest, document.notes[index].id)
    if (!Number.isSafeInteger(largest) || largest >= Number.MAX_SAFE_INTEGER)
        throw new Error("便签 ID 溢出")
    const note = {
        id: largest + 1,
        title: "桌面便签",
        body: "",
        visible: true,
        topmost: false,
        x: 0,
        y: 0,
        width: 360,
        height: 360
    }
    document.notes.push(note)
    return note.id
}

function updateNote(document, id, title, body) {
    const note = findNote(document, id)
    if (!note)
        throw new Error("找不到便签")
    note.title = stringValue(title, 150) || "桌面便签"
    note.body = stringValue(body, 200000)
}

function deleteNote(document, id) {
    for (let index = 0; index < document.notes.length; ++index) {
        if (document.notes[index].id === id) {
            document.notes.splice(index, 1)
            return
        }
    }
    throw new Error("找不到便签")
}

function nodeUnlocked(goal, node) {
    if (!goal || !node)
        return false
    if (goalTerminated(goal))
        return false
    for (let index = 0; index < node.requires.length; ++index) {
        const requirement = findGoalNode(goal, node.requires[index])
        if (!requirement || requirement.completed_at === null)
            return false
    }
    return true
}

function goalProgress(goal) {
    if (!goal || goal.nodes.length === 0)
        return { completed: 0, total: 0, ratio: 0 }
    let completed = 0
    for (let index = 0; index < goal.nodes.length; ++index) {
        if (goal.nodes[index].completed_at !== null)
            ++completed
    }
    return { completed: completed, total: goal.nodes.length, ratio: completed / goal.nodes.length }
}

function cleanGoalText(value, limit) {
    return stringValue(value, limit).trim()
}

function addGoal(document, title, description, timestamp) {
    if (document.goals.length >= 100)
        throw new Error("大目标已达到 100 个上限")
    const cleanTitle = cleanGoalText(title, 200)
    if (cleanTitle.length === 0)
        throw new Error("请输入大目标名称")
    if (!Number.isSafeInteger(document.next_goal_id) || document.next_goal_id >= Number.MAX_SAFE_INTEGER)
        throw new Error("大目标 ID 溢出")
    const id = document.next_goal_id++
    document.goals.push({
        id: id,
        title: cleanTitle,
        description: cleanGoalText(description, 2000),
        created_at: Number.isFinite(timestamp) ? timestamp : now(),
        completed_at: null,
        terminated_at: null,
        nodes: []
    })
    return id
}

function updateGoal(document, goalId, title, description) {
    const goal = findGoal(document, goalId)
    if (!goal)
        throw new Error("找不到大目标")
    const cleanTitle = cleanGoalText(title, 200)
    if (cleanTitle.length === 0)
        throw new Error("请输入大目标名称")
    goal.title = cleanTitle
    goal.description = cleanGoalText(description, 2000)
}

function deleteGoal(document, goalId) {
    for (let index = 0; index < document.goals.length; ++index) {
        if (document.goals[index].id === goalId) {
            document.goals.splice(index, 1)
            return
        }
    }
    throw new Error("找不到大目标")
}

function toggleGoalTermination(document, goalId, timestamp) {
    const goal = findGoal(document, goalId)
    if (!goal)
        throw new Error("找不到大目标")
    if (goalTerminated(goal)) {
        goal.terminated_at = null
        syncGoalCompletion(goal, timestamp)
        return false
    }
    if (goal.completed_at !== null)
        throw new Error("已正常完成的大目标无需终止")
    goal.terminated_at = Number.isFinite(timestamp) ? timestamp : now()
    goal.completed_at = null
    return true
}

function addGoalNode(document, goalId, title, description, requires, shape, timestamp) {
    const goal = findGoal(document, goalId)
    if (!goal)
        throw new Error("找不到大目标")
    if (goalTerminated(goal))
        throw new Error("大目标已终止，请先恢复后再修改路线")
    let totalNodes = 0
    for (let index = 0; index < document.goals.length; ++index)
        totalNodes += document.goals[index].nodes.length
    if (totalNodes >= 2000)
        throw new Error("小目标已达到 2000 个上限")
    const cleanTitle = cleanGoalText(title, 200)
    if (cleanTitle.length === 0)
        throw new Error("请输入小目标名称")
    const uniqueRequires = []
    for (let index = 0; index < requires.length; ++index) {
        const required = Number(requires[index])
        if (!findGoalNode(goal, required))
            throw new Error("小目标包含不存在的前置条件")
        if (uniqueRequires.indexOf(required) < 0)
            uniqueRequires.push(required)
    }
    const cleanShape = Number.isInteger(shape) && shape >= -1 && shape <= 2 ? shape : -1
    if (!Number.isSafeInteger(document.next_node_id) || document.next_node_id >= Number.MAX_SAFE_INTEGER)
        throw new Error("小目标 ID 溢出")
    const id = document.next_node_id++
    goal.nodes.push({
        id: id,
        title: cleanTitle,
        description: cleanGoalText(description, 2000),
        requires: uniqueRequires,
        shape: cleanShape,
        created_at: Number.isFinite(timestamp) ? timestamp : now(),
        completed_at: null
    })
    goal.completed_at = null
    return id
}

function goalNodeRequirementError(goal, nodeId, requires) {
    const node = findGoalNode(goal, nodeId)
    if (!node)
        return "找不到小目标"
    if (!Array.isArray(requires))
        return "前置条件格式无效"

    const uniqueRequires = []
    for (let index = 0; index < requires.length; ++index) {
        const required = Number(requires[index])
        if (!Number.isSafeInteger(required) || !findGoalNode(goal, required))
            return "小目标包含不存在的前置条件"
        if (required === nodeId)
            return "小目标不能将自己设为前置条件"
        if (uniqueRequires.indexOf(required) < 0)
            uniqueRequires.push(required)
    }

    function pathToNode(fromId, targetId, visited) {
        if (fromId === targetId)
            return [fromId]
        if (visited[fromId])
            return null
        visited[fromId] = true
        const from = findGoalNode(goal, fromId)
        for (let index = 0; from && index < from.requires.length; ++index) {
            const path = pathToNode(from.requires[index], targetId, visited)
            if (path)
                return [fromId].concat(path)
        }
        return null
    }

    for (let index = 0; index < uniqueRequires.length; ++index) {
        const required = uniqueRequires[index]
        if (pathToNode(required, nodeId, {})) {
            const requiredNode = findGoalNode(goal, required)
            return "“" + requiredNode.title + "”已依赖当前节点，不能设为前置条件（会形成回环）"
        }
    }
    return ""
}

function updateGoalNode(document, goalId, nodeId, title, description, shape, requires) {
    const goal = findGoal(document, goalId)
    if (!goal)
        throw new Error("找不到大目标")
    if (goalTerminated(goal))
        throw new Error("大目标已终止，请先恢复后再修改路线")
    const node = findGoalNode(goal, nodeId)
    if (!node)
        throw new Error("找不到小目标")
    const cleanTitle = cleanGoalText(title, 200)
    if (cleanTitle.length === 0)
        throw new Error("请输入小目标名称")
    let uniqueRequires = null
    if (requires !== undefined) {
        const requirementError = goalNodeRequirementError(goal, nodeId, requires)
        if (requirementError.length > 0)
            throw new Error(requirementError)
        uniqueRequires = []
        for (let index = 0; index < requires.length; ++index) {
            const required = Number(requires[index])
            if (uniqueRequires.indexOf(required) < 0)
                uniqueRequires.push(required)
        }
    }
    node.title = cleanTitle
    node.description = cleanGoalText(description, 2000)
    node.shape = Number.isInteger(shape) && shape >= -1 && shape <= 2 ? shape : -1
    if (uniqueRequires !== null) {
        node.requires = uniqueRequires
        node.completed_at = null
        syncGoalCompletion(goal)
    }
}

function goalNodeDeletionError(goal, nodeId) {
    if (goalTerminated(goal))
        return "大目标已终止，请先恢复后再修改路线"
    if (!findGoalNode(goal, nodeId))
        return "找不到小目标"
    for (let index = 0; index < goal.nodes.length; ++index) {
        if (goal.nodes[index].requires.indexOf(nodeId) >= 0)
            return "该节点仍是其他小目标的前置条件，无法删除"
    }
    return ""
}

function syncGoalCompletion(goal, timestamp) {
    if (goalTerminated(goal)) {
        goal.completed_at = null
        return
    }
    const progress = goalProgress(goal)
    if (progress.total > 0 && progress.completed === progress.total) {
        if (goal.completed_at === null)
            goal.completed_at = Number.isFinite(timestamp) ? timestamp : now()
    } else {
        goal.completed_at = null
    }
}

function deleteGoalNode(document, goalId, nodeId, timestamp) {
    const goal = findGoal(document, goalId)
    if (!goal)
        throw new Error("找不到大目标")
    const error = goalNodeDeletionError(goal, nodeId)
    if (error.length > 0)
        throw new Error(error)
    for (let index = 0; index < goal.nodes.length; ++index) {
        if (goal.nodes[index].id === nodeId) {
            goal.nodes.splice(index, 1)
            syncGoalCompletion(goal, timestamp)
            return
        }
    }
}

function toggleGoalNode(document, goalId, nodeId, timestamp) {
    const goal = findGoal(document, goalId)
    const node = findGoalNode(goal, nodeId)
    if (!goal || !node)
        throw new Error("找不到小目标")
    if (goalTerminated(goal))
        throw new Error("大目标已终止，请先恢复后再继续")
    const completing = node.completed_at === null
    if (completing && !nodeUnlocked(goal, node))
        throw new Error("请先完成前置小目标")
    if (!completing) {
        for (let index = 0; index < goal.nodes.length; ++index) {
            const dependent = goal.nodes[index]
            if (dependent.completed_at !== null && dependent.requires.indexOf(nodeId) >= 0)
                throw new Error("已有完成节点依赖它，暂时不能撤回")
        }
    }
    const changedAt = Number.isFinite(timestamp) ? timestamp : now()
    node.completed_at = completing ? changedAt : null
    syncGoalCompletion(goal, changedAt)
    return completing
}

function goalLayout(goal, nodeWidth, nodeHeight, horizontalGap, verticalGap) {
    if (!goal || goal.nodes.length === 0)
        return { nodes: [], edges: [], width: nodeWidth, height: nodeHeight }
    const levels = {}
    function levelFor(node) {
        if (levels[node.id] !== undefined)
            return levels[node.id]
        let lowestLevel = null
        let highestLevel = 0
        for (let index = 0; index < node.requires.length; ++index) {
            const requirement = findGoalNode(goal, node.requires[index])
            if (requirement) {
                const requirementLevel = levelFor(requirement)
                lowestLevel = lowestLevel === null
                              ? requirementLevel : Math.min(lowestLevel, requirementLevel)
                highestLevel = Math.max(highestLevel, requirementLevel)
            }
        }
        // Stage-priority ranking: a join remains alongside its latest
        // prerequisite when another branch is still catching up. This keeps
        // parallel milestones aligned without drawing an edge upwards.
        const level = lowestLevel === null ? 0 : Math.max(lowestLevel + 1, highestLevel)
        levels[node.id] = level
        return level
    }
    const groups = []
    const items = {}
    const realItemIds = {}
    function addItem(id, level, node) {
        const item = { id: id, level: level, node: node, initialOrder: groups[level].length }
        groups[level].push(item)
        items[id] = item
        return item
    }
    for (let index = 0; index < goal.nodes.length; ++index) {
        const level = levelFor(goal.nodes[index])
        if (!groups[level])
            groups[level] = []
        const itemId = "node:" + goal.nodes[index].id
        addItem(itemId, level, goal.nodes[index])
        realItemIds[goal.nodes[index].id] = itemId
    }
    const previous = {}
    const following = {}
    function connect(sourceId, targetId) {
        if (!following[sourceId])
            following[sourceId] = []
        if (!previous[targetId])
            previous[targetId] = []
        following[sourceId].push(targetId)
        previous[targetId].push(sourceId)
    }
    const edgeSpecs = []
    let edgeNumber = 0
    const sameRankLanes = {}
    for (let targetIndex = 0; targetIndex < goal.nodes.length; ++targetIndex) {
        const target = goal.nodes[targetIndex]
        for (let index = 0; index < target.requires.length; ++index) {
            const sourceId = target.requires[index]
            const source = findGoalNode(goal, sourceId)
            if (!source)
                continue
            const dummyIds = []
            let previousId = realItemIds[sourceId]
            for (let level = levels[sourceId] + 1; level < levels[target.id]; ++level) {
                if (!groups[level])
                    groups[level] = []
                const dummyId = "edge:" + sourceId + ":" + target.id + ":" + edgeNumber + ":" + level
                addItem(dummyId, level, null)
                connect(previousId, dummyId)
                previousId = dummyId
                dummyIds.push(dummyId)
            }
            connect(previousId, realItemIds[target.id])
            const sameRank = levels[sourceId] === levels[target.id]
            const lane = sameRank ? (sameRankLanes[levels[target.id]] || 0) : -1
            if (sameRank)
                sameRankLanes[levels[target.id]] = lane + 1
            edgeSpecs.push({ sourceId: sourceId, targetId: target.id,
                             dummyIds: dummyIds, sameRank: sameRank, lane: lane })
            ++edgeNumber
        }
    }

    // Sugiyama-style ordering: virtual nodes turn each edge into single-rank
    // segments, then alternating barycenter sweeps keep related paths close.
    function orderFor(id) {
        return items[id] ? items[id].order : 0
    }
    function reorder(level, neighbours) {
        const group = groups[level] || []
        group.sort(function(left, right) {
            const leftNeighbours = (neighbours[left.id] || []).filter(function(id) {
                return items[id] && items[id].level !== level
            })
            const rightNeighbours = (neighbours[right.id] || []).filter(function(id) {
                return items[id] && items[id].level !== level
            })
            function barycenter(ids) {
                if (ids.length === 0)
                    return null
                let total = 0
                for (let index = 0; index < ids.length; ++index)
                    total += orderFor(ids[index])
                return total / ids.length
            }
            const leftCenter = barycenter(leftNeighbours)
            const rightCenter = barycenter(rightNeighbours)
            if (leftCenter === null && rightCenter === null)
                return left.initialOrder - right.initialOrder
            if (leftCenter === null)
                return 1
            if (rightCenter === null)
                return -1
            return leftCenter === rightCenter
                   ? left.initialOrder - right.initialOrder : leftCenter - rightCenter
        })
        for (let index = 0; index < group.length; ++index)
            group[index].order = index
    }
    function enforceSameRankOrder(level) {
        const group = groups[level] || []
        const order = {}
        const pending = {}
        const children = {}
        for (let index = 0; index < group.length; ++index) {
            const id = group[index].id
            order[id] = index
            pending[id] = 0
            children[id] = []
        }
        for (let index = 0; index < group.length; ++index) {
            const targetId = group[index].id
            const parents = previous[targetId] || []
            for (let parentIndex = 0; parentIndex < parents.length; ++parentIndex) {
                const sourceId = parents[parentIndex]
                if (items[sourceId] && items[sourceId].level === level) {
                    ++pending[targetId]
                    children[sourceId].push(targetId)
                }
            }
        }
        const sorted = []
        while (sorted.length < group.length) {
            let nextId = null
            for (let index = 0; index < group.length; ++index) {
                const candidate = group[index].id
                if (pending[candidate] === 0
                    && (nextId === null || order[candidate] < order[nextId]))
                    nextId = candidate
            }
            if (nextId === null)
                break
            pending[nextId] = -1
            sorted.push(items[nextId])
            for (let index = 0; index < children[nextId].length; ++index)
                --pending[children[nextId][index]]
        }
        if (sorted.length === group.length) {
            groups[level] = sorted
            for (let index = 0; index < sorted.length; ++index)
                sorted[index].order = index
        }
    }
    for (let level = 0; level < groups.length; ++level) {
        const group = groups[level] || []
        for (let index = 0; index < group.length; ++index)
            group[index].order = index
        enforceSameRankOrder(level)
    }
    for (let sweep = 0; sweep < 4; ++sweep) {
        for (let level = 1; level < groups.length; ++level) {
            reorder(level, previous)
            enforceSameRankOrder(level)
        }
        for (let level = groups.length - 2; level >= 0; --level) {
            reorder(level, following)
            enforceSameRankOrder(level)
        }
    }

    let widest = 1
    for (let level = 0; level < groups.length; ++level)
        widest = Math.max(widest, (groups[level] || []).length)
    const width = widest * nodeWidth + Math.max(0, widest - 1) * horizontalGap
    const topPadding = verticalGap
    const arranged = []
    const positions = {}
    for (let level = 0; level < groups.length; ++level) {
        const group = groups[level] || []
        const groupWidth = group.length * nodeWidth + Math.max(0, group.length - 1) * horizontalGap
        const offset = (width - groupWidth) / 2
        for (let column = 0; column < group.length; ++column) {
            const item = group[column]
            const centerX = offset + column * (nodeWidth + horizontalGap) + nodeWidth / 2
            const y = topPadding + level * (nodeHeight + verticalGap)
            positions[item.id] = { x: centerX, y: y }
            if (item.node) {
                arranged.push({ node: item.node, x: centerX - nodeWidth / 2,
                                y: y, level: level })
            }
        }
    }
    const edges = []
    for (let index = 0; index < edgeSpecs.length; ++index) {
        const spec = edgeSpecs[index]
        const waypoints = []
        for (let dummyIndex = 0; dummyIndex < spec.dummyIds.length; ++dummyIndex) {
            const point = positions[spec.dummyIds[dummyIndex]]
            if (point)
                waypoints.push(point)
        }
        edges.push({ sourceId: spec.sourceId, targetId: spec.targetId,
                     waypoints: waypoints, sameRank: spec.sameRank, lane: spec.lane })
    }
    return {
        nodes: arranged,
        edges: edges,
        width: width,
        height: topPadding + groups.length * nodeHeight + Math.max(0, groups.length - 1) * verticalGap
    }
}

function routeGoalEdges(layout, nodeWidth, nodeHeight, horizontalGap, verticalGap) {
    if (!layout || !Array.isArray(layout.nodes) || !Array.isArray(layout.edges))
        return []
    // Grid routing is deliberately capped: a dense graph is still readable with
    // the layered fallback, while an unbounded A* search would harm the widget.
    if (layout.nodes.length > 180)
        return layout.edges.map(function(edge) { return { edge: edge, points: [] } })

    const grid = Math.max(6, Math.min(nodeWidth * 0.14, horizontalGap * 0.35, verticalGap * 0.35))
    const width = Math.max(3, Math.ceil(layout.width / grid) + 3)
    const height = Math.max(3, Math.ceil(layout.height / grid) + 3)
    if (width * height > 48000)
        return layout.edges.map(function(edge) { return { edge: edge, points: [] } })
    const blocked = new Array(width * height).fill(false)
    const occupied = new Array(width * height).fill(0)
    const byNodeId = {}
    const clearance = Math.max(2, grid * 0.35)
    for (let index = 0; index < layout.nodes.length; ++index) {
        const entry = layout.nodes[index]
        byNodeId[entry.node.id] = entry
        const firstX = Math.max(0, Math.floor((entry.x - clearance) / grid))
        const lastX = Math.min(width - 1, Math.ceil((entry.x + nodeWidth + clearance) / grid))
        const firstY = Math.max(0, Math.floor((entry.y - clearance) / grid))
        const lastY = Math.min(height - 1, Math.ceil((entry.y + nodeHeight + clearance) / grid))
        for (let y = firstY; y <= lastY; ++y) {
            for (let x = firstX; x <= lastX; ++x)
                blocked[y * width + x] = true
        }
    }
    function pointIndex(point) {
        return point.y * width + point.x
    }
    function gridPoint(point) {
        return {
            x: Math.max(0, Math.min(width - 1, Math.round(point.x / grid))),
            y: Math.max(0, Math.min(height - 1, Math.round(point.y / grid)))
        }
    }
    function drawingPoint(point) {
        return { x: point.x * grid, y: point.y * grid }
    }
    function pushHeap(heap, item) {
        heap.push(item)
        let index = heap.length - 1
        while (index > 0) {
            const parent = Math.floor((index - 1) / 2)
            if (heap[parent].score <= item.score)
                break
            heap[index] = heap[parent]
            index = parent
        }
        heap[index] = item
    }
    function popHeap(heap) {
        const result = heap[0]
        const last = heap.pop()
        if (heap.length === 0)
            return result
        let index = 0
        while (true) {
            let child = index * 2 + 1
            if (child >= heap.length)
                break
            if (child + 1 < heap.length && heap[child + 1].score < heap[child].score)
                ++child
            if (heap[child].score >= last.score)
                break
            heap[index] = heap[child]
            index = child
        }
        heap[index] = last
        return result
    }
    function route(start, end) {
        const startPoint = gridPoint(start)
        const endPoint = gridPoint(end)
        const startIndex = pointIndex(startPoint)
        const endIndex = pointIndex(endPoint)
        // Ports are always just outside their own obstacle, but explicitly
        // clear their cells to tolerate rounding on tight compact layouts.
        const startWasBlocked = blocked[startIndex]
        const endWasBlocked = blocked[endIndex]
        blocked[startIndex] = false
        blocked[endIndex] = false
        const costs = new Array(width * height).fill(Infinity)
        const parents = new Array(width * height).fill(-1)
        const directions = new Array(width * height).fill(-1)
        const heap = []
        costs[startIndex] = 0
        pushHeap(heap, { index: startIndex, score: 0 })
        const steps = [[1, 0], [-1, 0], [0, 1], [0, -1]]
        while (heap.length > 0) {
            const current = popHeap(heap)
            if (current.index === endIndex)
                break
            const currentCost = costs[current.index]
            const x = current.index % width
            const y = Math.floor(current.index / width)
            for (let direction = 0; direction < steps.length; ++direction) {
                const nextX = x + steps[direction][0]
                const nextY = y + steps[direction][1]
                if (nextX < 0 || nextY < 0 || nextX >= width || nextY >= height)
                    continue
                const nextIndex = nextY * width + nextX
                if (blocked[nextIndex])
                    continue
                const turning = directions[current.index] >= 0
                                && directions[current.index] !== direction ? 0.8 : 0
                const nextCost = currentCost + 1 + turning + occupied[nextIndex] * 3.5
                if (nextCost >= costs[nextIndex])
                    continue
                costs[nextIndex] = nextCost
                parents[nextIndex] = current.index
                directions[nextIndex] = direction
                const distance = Math.abs(nextX - endPoint.x) + Math.abs(nextY - endPoint.y)
                pushHeap(heap, { index: nextIndex, score: nextCost + distance })
            }
        }
        if (parents[endIndex] < 0 && endIndex !== startIndex) {
            blocked[startIndex] = startWasBlocked
            blocked[endIndex] = endWasBlocked
            return []
        }
        const path = []
        let currentIndex = endIndex
        while (currentIndex >= 0) {
            path.push(drawingPoint({ x: currentIndex % width,
                                     y: Math.floor(currentIndex / width) }))
            if (currentIndex === startIndex)
                break
            currentIndex = parents[currentIndex]
        }
        path.reverse()
        for (let index = 0; index < path.length; ++index) {
            const cell = gridPoint(path[index])
            ++occupied[pointIndex(cell)]
        }
        blocked[startIndex] = startWasBlocked
        blocked[endIndex] = endWasBlocked
        const simplified = []
        for (let index = 0; index < path.length; ++index) {
            const previous = simplified.length > 1 ? simplified[simplified.length - 2] : null
            const current = path[index]
            const last = simplified.length > 0 ? simplified[simplified.length - 1] : null
            if (previous && last
                && (previous.x === last.x && last.x === current.x
                    || previous.y === last.y && last.y === current.y))
                simplified[simplified.length - 1] = current
            else
                simplified.push(current)
        }
        return simplified
    }
    const routed = []
    for (let index = 0; index < layout.edges.length; ++index) {
        const edge = layout.edges[index]
        const source = byNodeId[edge.sourceId]
        const target = byNodeId[edge.targetId]
        if (!source || !target) {
            routed.push({ edge: edge, points: [] })
            continue
        }
        let sourcePort
        let targetPort
        let sourceAnchor
        let targetAnchor
        if (source.level === target.level) {
            const leftToRight = source.x <= target.x
            sourceAnchor = { x: source.x + (leftToRight ? nodeWidth : 0),
                             y: source.y + nodeHeight / 2 }
            targetAnchor = { x: target.x + (leftToRight ? 0 : nodeWidth),
                             y: target.y + nodeHeight / 2 }
            sourcePort = { x: sourceAnchor.x + (leftToRight ? clearance + grid : -clearance - grid),
                           y: sourceAnchor.y }
            targetPort = { x: targetAnchor.x + (leftToRight ? -clearance - grid : clearance + grid),
                           y: targetAnchor.y }
        } else {
            sourceAnchor = { x: source.x + nodeWidth / 2, y: source.y + nodeHeight }
            targetAnchor = { x: target.x + nodeWidth / 2, y: target.y }
            sourcePort = { x: sourceAnchor.x, y: sourceAnchor.y + clearance + grid }
            targetPort = { x: targetAnchor.x, y: targetAnchor.y - clearance - grid }
        }
        const gridPath = route(sourcePort, targetPort)
        const points = []
        function addPoint(point) {
            const last = points.length > 0 ? points[points.length - 1] : null
            if (!last || last.x !== point.x || last.y !== point.y)
                points.push(point)
        }
        function addOrthogonal(point, horizontalFirst) {
            const last = points.length > 0 ? points[points.length - 1] : null
            if (last && last.x !== point.x && last.y !== point.y) {
                addPoint(horizontalFirst ? { x: point.x, y: last.y }
                                         : { x: last.x, y: point.y })
            }
            addPoint(point)
        }
        if (gridPath.length > 0) {
            addPoint(sourceAnchor)
            addPoint(sourcePort)
            addOrthogonal(gridPath[0], true)
            for (let pointIndex = 1; pointIndex < gridPath.length; ++pointIndex)
                addPoint(gridPath[pointIndex])
            addOrthogonal(targetPort, false)
            addPoint(targetAnchor)
        }
        routed.push({ edge: edge, points: points })
    }
    return routed
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
