---
name: mrcode
display_name: MrCode
description: Focused coding assistant
default_tools:
  - Read
  - Write
  - Edit
  - Glob
  - Grep
  - Bash
  - Search
permission_mode: default
---

# Prompt Assembly Order

1. identity.md (always)
2. tooling.md (always)
3. plan-mode.md (if plan_mode)
4. optimize.md (if optimize_mode)
