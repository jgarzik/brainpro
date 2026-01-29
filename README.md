# brainpro

A local agentic coding assistant. Vendor-neutral, multi-model routing—send coding to Claude, planning to Qwen, exploration to GPT.<img width="401" height="387" alt="{FB75D017-3C33-4723-9372-A4980D3B7166}" src="https://github.com/user-attachments/assets/da60511a-496f-4ee0-802b-8d4725d03219" />


## Two Paths

| Path | Entry Point | Persona | Use Case |
|------|-------------|---------|----------|
| **Direct** | `yo` CLI | MrCode (7 tools) | Local dev, quick tasks |
| **Gateway** | `brainpro-gateway` + `brainpro-agent` | MrBot (12+ tools) | Remote access, daemon mode, Docker |

## Features

- **Local execution** - Runs on your machine, project-scoped file access
- **Multi-backend LLM** - Venice, OpenAI, Anthropic, Ollama, custom endpoints
- **Model routing** - Auto-select models by task type (planning/coding/exploration)
- **Built-in tools** - Read, Write, Edit, Grep, Glob, Bash, Search
- **MCP integration** - External tool servers via Model Context Protocol
- **Subagents** - Delegate to specialized agents with restricted tools
- **Skill packs** - Reusable instruction sets with tool restrictions
- **Permission system** - Granular allow/ask/deny rules
- **Session transcripts** - JSONL audit logs

## Quick Start

### Direct (yo)

```bash
cargo build --release
yo -p "explain main.rs"    # one-shot
yo                          # interactive REPL
```

### Gateway + Daemon (Docker)

```bash
docker-compose up -d
# Connect via WebSocket at ws://localhost:18789
```

## Documentation

- **[DESIGN.md](DESIGN.md)** - Technical architecture, protocols, internals
- **[USERGUIDE.md](USERGUIDE.md)** - Setup, configuration, security hardening

## Inspired By

- [Claude Code](https://github.com/anthropics/claude-code)
- [opencode](https://github.com/opencode-ai/opencode)
- [clawdbot](https://github.com/crjaensch/clawdbot)
