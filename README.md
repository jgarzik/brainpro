# brainpro

An open source, local agentic butler for software development. `brainpro` orchestrates LLM interactions with file and shell tools, providing a secure policy engine for automated coding tasks.

brainpro features **multi-vendor, multi-model routing**, sending coding tasks to the best coding model (anthropic opus, for example) and sending planning tasks to the best planning model (qwen, for example), taking a vendor neutral, "best model wins" approach.

## Status

Not production yet.  Currently undergoing heavy development and testing.  Contributions welcome (file an issue).

## Features

- **Local execution** - Runs on your machine with access restricted to project files
- **Multi-backend LLM support** - Venice (default), OpenAI, Anthropic, Ollama, or custom endpoints
- **Built-in tools** - Read, Write, Edit, Grep, Glob, Bash
- **MCP integration** - Connect external tool servers via Model Context Protocol
- **Subagents** - Delegate tasks to specialized agents with restricted tools
- **Skill Packs** - Reusable instruction sets with tool restrictions
- **Model Routing** - Automatic model selection based on task type
- **Permission system** - Granular allow/ask/deny rules for tool access
- **Session transcripts** - JSONL audit logs of all interactions
- **Context management** - Automatic compaction when conversation grows large

## Usage

### Installation

```bash
cargo build --release
```

### Running

```bash
# Interactive REPL
brainpro

# One-shot prompt
brainpro -p "your prompt here"

# With auto-approve for file edits
brainpro -p "refactor main.rs" --yes
```

### Environment Variables

| Variable | Backend |
|----------|---------|
| `VENICE_API_KEY` | Venice (default) |
| `OPENAI_API_KEY` | OpenAI |
| `ANTHROPIC_API_KEY` | Anthropic |

### CLI Options

| Flag | Description |
|------|-------------|
| `-p, --prompt` | One-shot prompt mode |
| `--target` | Override LLM target (format: `model@backend`) |
| `--mode` | Permission mode: default, acceptEdits, bypassPermissions |
| `--max-turns` | Max agent iterations per turn (default: 12) |
| `--trace` | Enable detailed tracing |
| `--list-targets` | Show configured backends and default target |

## REPL Commands

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/exit`, `/quit` | Exit REPL |
| `/clear` | Clear conversation history |
| `/session` | Show session ID and transcript path |
| `/context` | Show context usage stats |
| `/backends` | List configured backends |
| `/target [model@backend]` | Show or set current target |
| `/mode [name]` | Get or set permission mode |
| `/permissions` | Show permission rules |
| `/permissions add [allow\|ask\|deny] "pattern"` | Add rule |
| `/trace` | Toggle tracing |
| `/agents` | List available subagents |
| `/task <agent> <prompt>` | Run a subagent with the given prompt |
| `/skillpacks` | List available skill packs |
| `/skillpack use <name>` | Activate a skill pack |
| `/skillpack drop <name>` | Deactivate a skill pack |
| `/skillpack active` | List active skill packs |
| `/mcp list` | List MCP servers |
| `/mcp connect <name>` | Connect to MCP server |
| `/mcp disconnect <name>` | Disconnect MCP server |
| `/mcp tools <name>` | List tools from MCP server |
| `/compact` | Summarize older messages to reclaim context |
| `/commands` | List available slash commands |
| `/<name> [args]` | Run user-defined slash command |

## Configuration

Configuration hierarchy (highest to lowest priority):
1. CLI arguments
2. `.brainpro/config.local.toml` (git-ignored)
3. `.brainpro/config.toml` (project)
4. `~/.brainpro/config.toml` (user)
5. Built-in defaults

### Config Sections

```toml
[backends.venice]
base_url = "https://api.venice.ai/api/v1"
api_key_env = "VENICE_API_KEY"

default_target = "qwen3-235b-a22b-instruct-2507@venice"

[permissions]
mode = "default"
allow = ["Bash(git diff:*)"]
ask = ["Write"]
deny = ["Bash(rm -rf:*)"]

[bash]
timeout_ms = 120000
max_output_bytes = 200000

[context]
max_chars = 250000
auto_compact_enabled = true

[mcp.servers.calc]
command = "/path/to/mcp-calc"
transport = "stdio"  # or "http", "sse"
# url = "https://..."  # for http/sse transports
auto_start = false

[model_routing.routes]
planning = "qwen3-235b-a22b-instruct-2507@venice"
coding = "claude-3-5-sonnet-latest@claude"
exploration = "gpt-4o-mini@chatgpt"
```

See `example-brainpro.toml` for complete reference.

## Security Model

### Permission Modes

| Mode | Behavior |
|------|----------|
| `default` | Read-only tools allowed; Write/Edit/Bash require approval |
| `acceptEdits` | File mutations allowed; Bash requires approval |
| `bypassPermissions` | All tools allowed (trusted environments only) |

### Rule Patterns

- `"Write"` - Match all Write calls
- `"Bash(git:*)"` - Match Bash commands starting with "git"
- `"Bash(npm install)"` - Match exact command
- `"mcp.server.*"` - Match all tools from MCP server

### Built-in Protections

- `curl` and `wget` blocked by default
- All paths validated to stay within project root
- Symlinks resolved to prevent escape

## Subagents

Subagents allow delegating tasks to specialized agents with restricted tools and permissions.

### Agent Spec Format

Agent specs are stored in `.brainpro/agents/<name>.toml`:

```toml
name = "scout"
description = "Read-only repo scout: find files, summarize structure"
allowed_tools = ["Read", "Grep", "Glob"]
permission_mode = "default"
max_turns = 8
system_prompt = """
You are Scout, a read-only exploration agent.
Use Glob to find files, Grep to search, Read to examine.
"""

# Optional: override target for this agent
# target = "gpt-4o-mini@chatgpt"
```

### Built-in Agents

| Agent | Tools | Description |
|-------|-------|-------------|
| `scout` | Read, Grep, Glob | Read-only exploration |
| `patch` | Read, Grep, Glob, Edit, Write | Code editing |
| `test` | Read, Bash | Test execution |
| `docs` | Read, Write, Glob | Documentation writing |

### Using Subagents

**Via REPL:**
```
/agents                           # List available agents
/task scout find the config parser
```

**Via LLM (Task tool):**
The main agent can delegate using the `Task` tool:
```json
{
  "agent": "scout",
  "prompt": "Find where config parsing happens"
}
```

### Safety

- Subagents cannot spawn other subagents (no recursion)
- Permission mode is clamped to parent's mode (subagent cannot exceed parent permissions)
- Tool access is restricted to `allowed_tools` list
- Subagent activity is logged to transcripts

## Skill Packs

Skill packs are reusable instruction sets that guide the agent for specific tasks. They use SKILL.md files with YAML frontmatter for metadata.

### SKILL.md Format

Skill packs are stored in `.brainpro/skills/<name>/SKILL.md` or `~/.brainpro/skills/<name>/SKILL.md`:

```markdown
---
name: safe-file-reader
description: Read files without making changes
allowed-tools: Read, Grep, Glob
---

You are in safe-file-reader mode. Only inspect files; do not modify anything.
Use Glob to find files, Grep to search content, Read to examine.
```

### Frontmatter Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Lowercase letters, numbers, hyphens (max 64 chars) |
| `description` | Yes | Brief description (max 1024 chars) |
| `allowed-tools` | No | Restrict to specific tools (CSV or YAML list) |

### Using Skill Packs

**Via REPL:**
```
/skillpacks              # List available skill packs
/skillpack use reader    # Activate a skill pack
/skillpack active        # Show active skill packs
/skillpack drop reader   # Deactivate
```

**Via LLM (ActivateSkill tool):**
The agent can activate skills using the `ActivateSkill` tool, or by mentioning `$skill-name` in conversation.

### Tool Restrictions

When multiple skills are active, their `allowed-tools` are intersected. Only tools allowed by all active skills can be used.

## Slash Commands

User-defined commands stored as markdown files:
- `.brainpro/commands/<name>.md` (project)
- `~/.brainpro/commands/<name>.md` (user)

### Format

```markdown
---
description: Fix an issue by number
allowed_tools:
  - Read
  - Edit
---

Fix issue #$ARGUMENTS in the codebase. Read relevant files, make the fix.
```

Use `$ARGUMENTS` placeholder for user input. Frontmatter is optional.

### Usage

```
/fix-issue 123          # Runs fix-issue.md with "123" as $ARGUMENTS
/commands               # List available commands
```

## Model Routing

Model routing automatically selects the best model for each subagent based on task type. Different models excel at different tasks—planning, coding, exploration, etc.

### Route Categories

| Category | Keywords | Default Target |
|----------|----------|----------------|
| `planning` | plan, architect, design | qwen3-235b@venice |
| `coding` | patch, edit, code, implement | claude-3-5-sonnet@claude |
| `exploration` | scout, explore, find, search | gpt-4o-mini@chatgpt |
| `testing` | test, verify, check | gpt-4o-mini@chatgpt |
| `documentation` | doc, readme, comment | gpt-4o-mini@chatgpt |
| `fast` | (explicit) | gpt-4o-mini@chatgpt |
| `default` | (fallback) | gpt-4o-mini@chatgpt |

### How It Works

1. Subagent name/description is analyzed for keywords
2. Category is inferred from keywords
3. Target is resolved: explicit spec > config route > hardcoded default
4. Subagent runs on the selected model

### Configuration

Override defaults in config:

```toml
[model_routing.routes]
planning = "qwen3-235b-a22b-instruct-2507@venice"
coding = "claude-3-5-sonnet-latest@claude"
exploration = "gpt-4o-mini@chatgpt"
testing = "gpt-4o-mini@chatgpt"
```

Explicit `target` in agent specs always takes priority over routing.

## Architecture

```
User Input
    │
    ▼
┌─────────────────────────────────────────────────────┐
│  cli.rs                                             │
│  REPL loop, slash commands, message history         │
└─────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────┐
│  agent.rs                                           │
│  Core loop: LLM request → tool calls → results      │
│  Iterates until LLM stops requesting tools (max 12) │
└─────────────────────────────────────────────────────┘
    │
    ├──────────────────────────────────┐
    ▼                                  ▼
┌───────────────┐              ┌───────────────────┐
│  backend.rs   │              │  policy.rs        │
│  LLM registry │              │  Permission rules │
│  Lazy loading │              │  allow/ask/deny   │
└───────────────┘              └───────────────────┘
    │
    ▼
┌───────────────┐
│  llm.rs       │
│  HTTP client  │
│  OpenAI API   │
└───────────────┘
```

### Modules

| File | Responsibility |
|------|----------------|
| `main.rs` | Entry point, CLI parsing, config bootstrap |
| `cli.rs` | REPL interface, slash command dispatch |
| `agent.rs` | Agent loop, tool orchestration, LLM calls |
| `config.rs` | Hierarchical config loading and merging |
| `policy.rs` | Permission decision engine, rule matching |
| `backend.rs` | Backend registry, lazy client initialization |
| `llm.rs` | OpenAI-compatible HTTP client |
| `transcript.rs` | JSONL session logging |
| `compact.rs` | Context compaction via LLM summarization |
| `commands.rs` | Slash command loader and dispatch |
| `tools/mod.rs` | Tool registry, path validation, dispatch |
| `tools/read.rs` | Read file contents |
| `tools/write.rs` | Create/overwrite files |
| `tools/edit.rs` | Find-and-replace edits |
| `tools/bash.rs` | Shell command execution with timeout |
| `tools/grep.rs` | Regex content search |
| `tools/glob.rs` | File pattern matching |
| `tools/task.rs` | Subagent delegation tool |
| `tools/mcp_dispatch.rs` | Route MCP tool calls |
| `tools/activate_skill.rs` | Skill pack activation tool |
| `subagent.rs` | Subagent runtime, tool filtering, mode clamping |
| `skillpacks/mod.rs` | Skill pack module exports |
| `skillpacks/parser.rs` | SKILL.md file parser |
| `skillpacks/index.rs` | Skill pack discovery and indexing |
| `skillpacks/activation.rs` | Active skill lifecycle |
| `model_routing.rs` | Task-based model selection |
| `mcp/client.rs` | MCP JSON-RPC client |
| `mcp/manager.rs` | MCP server lifecycle |
| `mcp/transport.rs` | Transport layer (stdio, http, sse) |

### Data Flow

1. User input received (REPL or one-shot)
2. Agent adds message to conversation
3. Agent resolves target (model@backend)
4. Agent collects tool schemas (built-in + MCP)
5. LLM request sent with messages + tools
6. Response parsed for text and tool calls
7. Each tool call: policy check → execute → log
8. Results added to conversation
9. Loop continues until LLM stops calling tools
10. Final response displayed to user

### Transcripts

Sessions logged to `.brainpro/sessions/<uuid>.jsonl` with events:
- User/assistant messages
- Tool calls and results
- Permission decisions
- Subagent lifecycle (start, end, tool calls)
- Skill pack lifecycle (index built, activate, deactivate, parse errors)
- MCP server lifecycle
- Errors and metadata

## Inspired By

- [Claude Code](https://github.com/anthropics/claude-code)
- [opencode](https://github.com/opencode-ai/opencode)
- [clawdbot](https://github.com/crjaensch/clawdbot)
