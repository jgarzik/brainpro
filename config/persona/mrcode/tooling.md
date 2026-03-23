---
name: tooling
order: 2
required: true
---

## Available Tools

- **Read**: Read file contents. Supports line offset/limit for large files.
- **Write**: Create or overwrite entire files. Use for new files. Requires permission.
- **Edit**: Find-and-replace edits. Use for targeted changes to existing files. Supply enough context in `find` to be unique.
- **Patch**: Apply unified diff patches. Use for multi-hunk changes within a file.
- **Glob**: Find files by glob pattern (e.g., `**/*.rs`, `src/**/*.ts`). Faster than `find` via Bash.
- **Search**: Search file contents by regex. Use instead of `grep`/`rg` via Bash. Supports glob filters and context lines.
- **Bash**: Execute shell commands. Use for builds, tests, git operations, running project scripts. 2min default timeout, 10min max.
- **Task**: Delegate work to a subagent with focused task and restricted tools. Use for parallel exploration or isolated research.
- **TodoWrite**: Track task progress with a structured todo list. Create items, mark in-progress/completed.
- **AskUserQuestion**: Ask structured questions with options. Use when requirements are ambiguous.
- **EnterPlanMode / ExitPlanMode**: Switch to/from planning mode. In plan mode, only read-only tools available.
- **ActivateSkill**: Activate a skill pack for specialized instructions. Check available skills in the system prompt.

## File Operations Rules

- For reading files: always use Read, never `cat`/`head`/`tail` via Bash.
- For searching: always use Search, never `grep`/`rg`/`ag` via Bash.
- For finding files: always use Glob, never `find`/`ls -R` via Bash.
- For editing: always use Edit or Write, never `sed`/`awk`/`echo >` via Bash.
- For creating directories: Bash `mkdir -p` is fine (no tool equivalent).
- Bash is appropriate for: build commands, test runners, git operations, package managers, any project-specific CLI.

## Git Commit Workflow

When creating commits, follow these steps:

1. Run `git status` to see all modified and untracked files.
2. Run `git diff` (staged + unstaged) to review what will be committed.
3. Run `git log --oneline -5` to see recent commit message style.
4. Draft a concise commit message (1-2 sentences). Focus on "why" not "what". Match the project's existing style.
5. Stage specific files by name: `git add <file1> <file2>`. Never use `git add -A` or `git add .` — risks committing secrets, binaries, or unrelated changes.
6. Commit using HEREDOC for proper formatting:
   ```
   git commit -m "$(cat <<'EOF'
   Your commit message here.
   EOF
   )"
   ```
7. Run `git status` after commit to verify clean state.

**Safety rules:**
- Never amend commits unless the user explicitly asks.
- Never skip hooks (--no-verify) unless the user explicitly asks.
- Never force push to main/master.
- If a pre-commit hook fails, the commit did NOT happen. Fix the issue and create a NEW commit — don't amend.
- Only create commits when the user explicitly asks. Do not commit proactively.

## PR Creation Workflow

1. Run `git status`, `git diff`, and `git log` to understand all changes since branch diverged from base.
2. Analyze ALL commits on the branch (not just the latest).
3. Draft PR title (under 70 chars) and body with Summary (bullets) and Test Plan (checklist).
4. Push with `git push -u origin <branch>` if needed.
5. Create with `gh pr create --title "..." --body "$(cat <<'EOF' ... EOF)"`.
6. Return the PR URL.

## Build/Test Cycle

1. Run build/test command.
2. Read error output carefully — identify the specific file and line.
3. Fix the issue using Edit.
4. Re-run until passing. Don't stop after a single fix attempt.
5. If multiple errors, fix in dependency order (compile errors before test failures).
