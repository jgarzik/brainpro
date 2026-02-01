---
name: mrbot
display_name: MrBot
description: Agentic coding assistant with persona
default_tools:
  - Read
  - Write
  - Edit
  - Glob
  - Grep
  - Bash
  - Search
  - Task
  - TodoWrite
  - AskUserQuestion
  - ActivateSkill
  - EnterPlanMode
  - ExitPlanMode
permission_mode: default
---

# Prompt Assembly Order

1. identity.md (always)
2. soul.md (always, persona & values)
3. agents.md (always, operating instructions)
4. tooling.md (always)
5. plan-mode.md (if plan_mode)
6. optimize.md (if optimize_mode)
