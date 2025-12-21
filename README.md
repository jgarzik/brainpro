# yo

An open source, local agentic butler for software development. `yo` orchestrates LLM interactions with file and shell tools, providing a secure policy engine for automated coding tasks.

## Features

- **Local execution** - Runs on your machine with access restricted to project files
- **Multi-backend LLM support** - Venice (default), OpenAI, Anthropic, Ollama, or custom endpoints
- **Built-in tools** - Read, Write, Edit, Grep, Glob, Bash
- **MCP integration** - Connect external tool servers via Model Context Protocol
- **Subagents** - Delegate tasks to specialized agents with restricted tools
- **Permission system** - Granular allow/ask/deny rules for tool access
- **Skill routing** - Map named skills to specific model@backend targets
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
yo

# One-shot prompt
yo -p "your prompt here"

# With auto-approve for file edits
yo -p "refactor main.rs" --yes
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
| `--list-targets` | Show configured backends and skills |

## REPL Commands

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/exit`, `/quit` | Exit REPL |
| `/clear` | Clear conversation history |
| `/session` | Show session ID and transcript path |
| `/context` | Show context usage stats |
| `/backends` | List configured backends |
| `/skills` | List available skills |
| `/skill [name]` | Get or set current skill |
| `/target [model@backend]` | Override current target |
| `/mode [name]` | Get or set permission mode |
| `/permissions` | Show permission rules |
| `/permissions add [allow\|ask\|deny] "pattern"` | Add rule |
| `/trace` | Toggle tracing |
| `/agents` | List available subagents |
| `/task <agent> <prompt>` | Run a subagent with the given prompt |
| `/mcp list` | List MCP servers |
| `/mcp connect <name>` | Connect to MCP server |
| `/mcp disconnect <name>` | Disconnect MCP server |
| `/mcp tools <name>` | List tools from MCP server |

## Configuration

Configuration hierarchy (highest to lowest priority):
1. CLI arguments
2. `.yo/config.local.toml` (git-ignored)
3. `.yo/config.toml` (project)
4. `~/.yo/config.toml` (user)
5. Built-in defaults

### Config Sections

```toml
[backends.venice]
base_url = "https://api.venice.ai/api/v1"
api_key_env = "VENICE_API_KEY"

[skills]
default = "qwen3-235b-a22b-instruct-2507@venice"
fast = "gpt-4o-mini@chatgpt"

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
args = []
auto_start = false
```

See `example-yo.toml` for complete reference.

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

Agent specs are stored in `.yo/agents/<name>.toml`:

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

# Optional: override skill or target
# skill = "fast"
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
| `context.rs` | Conversation history, compaction framework |
| `tools/mod.rs` | Tool registry, path validation, dispatch |
| `tools/read.rs` | Read file contents |
| `tools/write.rs` | Create/overwrite files |
| `tools/edit.rs` | Find-and-replace edits |
| `tools/bash.rs` | Shell command execution with timeout |
| `tools/grep.rs` | Regex content search |
| `tools/glob.rs` | File pattern matching |
| `tools/task.rs` | Subagent delegation tool |
| `tools/mcp_dispatch.rs` | Route MCP tool calls |
| `subagent.rs` | Subagent runtime, tool filtering, mode clamping |
| `mcp/client.rs` | MCP JSON-RPC client |
| `mcp/manager.rs` | MCP server lifecycle |
| `mcp/transport.rs` | Stdio transport layer |

### Data Flow

1. User input received (REPL or one-shot)
2. Agent adds message to conversation
3. Agent resolves skill → target (model@backend)
4. Agent collects tool schemas (built-in + MCP)
5. LLM request sent with messages + tools
6. Response parsed for text and tool calls
7. Each tool call: policy check → execute → log
8. Results added to conversation
9. Loop continues until LLM stops calling tools
10. Final response displayed to user

### Transcripts

Sessions logged to `.yo/sessions/<uuid>.jsonl` with events:
- User/assistant messages
- Tool calls and results
- Permission decisions
- Subagent lifecycle (start, end, tool calls)
- MCP server lifecycle
- Errors and metadata
