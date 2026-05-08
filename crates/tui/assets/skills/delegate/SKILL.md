---
name: delegate
description: Strategic task delegation. Plan and reason first, then offload laborious execution work (scaffolding, code generation, boilerplate, search, tests, file creation) to deepseek-v4-flash sub-agents. Use when the user asks for multi-step implementation tasks, code generation, refactoring, or any request that involves both planning and execution phases.
---

# Delegate

Plan the work, then ship the labor to `deepseek-v4-flash` sub-agents.

## Core Principle

The coordinating agent (you) handles reasoning, architecture, design decisions, and integration. Sub-agents running `deepseek-v4-flash` handle execution: scaffold generation, boilerplate, code writing, search/grep, test creation, and file edits.

This gives the user Pro-quality reasoning with Flash-level cost on execution — each flash sub-agent costs ~10x less per token than Pro.

## What to Delegate vs Keep

| Keep in parent (Pro / current model) | Delegate to flash sub-agents |
|---|---|
| Understanding the user's request | Generating scaffold files |
| Architecture and design decisions | Writing boilerplate code |
| Trade-off analysis | Creating new files from scratch |
| Security review | Search and grep across the codebase |
| Integration across modules | Running tests and collecting results |
| Synthesis of sub-agent results | Reading and summarizing multiple files |
| Final quality check | Bulk edits following a clear spec |
| Ambiguous decisions | Deterministic code generation |

Rule of thumb: if the task is well-specified, repetitive, or purely mechanical — delegate. If it requires judgment, trade-offs, or cross-cutting understanding — keep it.

## Workflow

### 1. Understand and Plan

Read the user's request. Reason about what's needed. Identify:

- What requires architectural thinking (keep)
- What is mechanical execution (delegate)
- Dependencies between sub-tasks

Use `checklist_write` to lay out the plan. Mark items as delegate vs direct.

### 2. Identify Delegation Units

Split execution work into independent, self-contained units. Each unit should be:

- **Self-contained**: has all context needed to complete (include file paths, specs, conventions)
- **Independent**: doesn't depend on another sub-agent's output
- **Verifiable**: has a clear acceptance criterion

Good delegation prompts are specific and bounded:

```
"Create src/auth/login.rs with a LoginForm struct containing
email and password fields. Derive Serialize, Deserialize, Debug.
Add validation that email is non-empty and password is >= 8 chars."
```

Bad (vague, needs judgment):

```
"Write the auth module."
```

### 3. Spawn Flash Sub-Agents

Use `agent_spawn` with `model: "deepseek-v4-flash"` for each independent unit.
Spawn them together in one turn for parallel execution:

```json
// agent_spawn call 1
{
  "prompt": "Create src/models/user.rs: User struct with id, name, email...",
  "model": "deepseek-v4-flash",
  "type": "implementer"
}

// agent_spawn call 2 (same turn — runs in parallel)
{
  "prompt": "Create src/models/post.rs: Post struct with id, title, body...",
  "model": "deepseek-v4-flash",
  "type": "implementer"
}
```

Key parameters:

- **`model`**: always `"deepseek-v4-flash"` for execution work
- **`type`**: `"implementer"` for code generation, `"explore"` for read-only search/investigation, `"general"` for mixed work
- **`cwd`**: set to the workspace root when the child needs file access
- **`allowed_tools`**: narrow to only what's needed (e.g., `["read_file", "write_file", "grep_files"]` for code gen)

### 4. Wait and Synthesize

Use `agent_wait` to collect results from parallel sub-agents. Then:

1. Verify each sub-agent's output (don't trust blindly)
2. Cross-check one finding against a direct `read_file`
3. Integrate results into a coherent whole
4. Run verification gates: `cargo check`, `cargo test`, etc.

### 5. Iterate if Needed

If a sub-agent's output needs adjustment, either:
- Fix it directly (small tweaks)
- Spawn a follow-up flash sub-agent with the correction (non-trivial rework)

## Parallel Delegation Pattern

Batch independent spawns in a single turn. The dispatcher runs them concurrently:

```
Turn N:
  - agent_spawn: create user.rs (flash)
  - agent_spawn: create post.rs (flash)
  - agent_spawn: create error.rs (flash)
  → all three run in parallel

Turn N+1:
  - agent_wait: all three
  - cargo check to verify
  - read_file to spot-check
  - synthesize results for the user
```

Don't serialize when you can parallelize. Three flash sub-agents in one turn finish faster and cost the same as three sequential turns.

## What NOT to Delegate

- The initial understanding and planning phase (you need full context)
- Architecture decisions that span modules
- Security-sensitive code paths
- Final integration and verification
- Tasks where the spec is ambiguous and needs judgment
- Very small tasks (sub-agent overhead isn't worth it for a single-line edit)

## Cost Awareness

Flash is ~10x cheaper than Pro per token. A typical flash sub-agent doing file creation costs pennies. The parent turn (planning + synthesis) is where the expensive reasoning lives.

Batch parallel sub-agents when possible — the cost is the same as sequential but the user waits less.

## Quick Reference

```
# Spawn a flash sub-agent for code generation
agent_spawn {
  prompt: "detailed, self-contained task description",
  model: "deepseek-v4-flash",
  type: "implementer"
}

# Spawn a flash sub-agent for search/investigation
agent_spawn {
  prompt: "search task description",
  model: "deepseek-v4-flash",
  type: "explore"
}

# Wait for all running sub-agents
agent_wait { wait_mode: "all" }

# Get a specific result
agent_result { agent_id: "...", block: true }
```
