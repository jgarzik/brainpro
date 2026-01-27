# brainpro User Guide

Setup, configuration, and usage.

## Part 1: Getting Started

brainpro is a local agentic coding assistant that orchestrates LLM interactions with file and shell tools. It runs on your machine, keeps data local, and supports multiple LLM backends.

### Installation

```bash
cargo build --release
```

Binaries:
- `yo` - Direct CLI (MrCode persona)
- `brainpro-gateway` - WebSocket gateway
- `brainpro-agent` - Agent daemon

### First Steps with `yo`

```bash
# One-shot prompt
yo -p "explain main.rs"

# Interactive REPL
yo

# Auto-approve file edits
yo -p "refactor main.rs" --yes
```

### Environment Variables

| Variable | Backend |
|----------|---------|
| `VENICE_API_KEY` | Venice (default) |
| `OPENAI_API_KEY` | OpenAI |
| `ANTHROPIC_API_KEY` | Anthropic |

### Example Workflows

```bash
# Code review
yo -p "review src/agent.rs for bugs and security issues"

# Refactoring
yo -p "extract the config parsing into a separate module"

# Test generation
yo -p "write unit tests for the policy module"

# Exploration
yo -p "how does the permission system work?"
```

---

## Part 2: The `yo` CLI

### CLI Flags

| Flag | Description |
|------|-------------|
| `-p, --prompt` | One-shot prompt (exit after response) |
| `--target` | Override LLM target (`model@backend`) |
| `--mode` | Permission mode: `default`, `acceptEdits`, `bypassPermissions` |
| `--max-turns` | Max agent iterations (default: 12) |
| `--trace` | Enable detailed tracing |
| `--list-targets` | Show configured backends |

### REPL Commands

| Command | Description |
|---------|-------------|
| `/help` | Show commands |
| `/exit`, `/quit` | Exit |
| `/clear` | Clear conversation |
| `/session` | Show session ID |
| `/context` | Context usage stats |
| `/compact` | Summarize old messages to reclaim context |
| `/target [model@backend]` | Show/set target |
| `/mode [name]` | Show/set permission mode |
| `/permissions` | Show permission rules |
| `/permissions add [allow\|ask\|deny] "pattern"` | Add rule |
| `/agents` | List subagents |
| `/task <agent> <prompt>` | Run subagent |
| `/skillpacks` | List skill packs |
| `/skillpack use <name>` | Activate skill |
| `/skillpack drop <name>` | Deactivate skill |
| `/mcp list` | List MCP servers |
| `/mcp connect <name>` | Connect MCP server |
| `/commands` | List slash commands |
| `/<name> [args]` | Run user-defined command |

### Configuration Basics

Config files (highest priority first):
1. CLI arguments
2. `.brainpro/config.local.toml` (git-ignored)
3. `.brainpro/config.toml` (project)
4. `~/.brainpro/config.toml` (user)
5. Built-in defaults

Minimal config:
```toml
default_target = "claude-3-5-sonnet-latest@claude"

[backends.claude]
base_url = "https://api.anthropic.com/v1"
api_key_env = "ANTHROPIC_API_KEY"
```

---

## Part 3: Docker/Gateway Setup

Use the gateway when you need:
- Remote access to the agent
- Persistent daemon process
- Multiple clients sharing one agent
- Container isolation

### Quick Start

```bash
docker-compose up -d
```

This starts:
- `brainpro-agent` listening on Unix socket
- `brainpro-gateway` exposing WebSocket on port 18789

Connect via WebSocket at `ws://localhost:18789`.

### Container Storage Model

The container uses a read-only filesystem with specific writable mount points:

| Path | Type | Purpose |
|------|------|---------|
| `/app/workspace` | bind mount | User project files (Write tool operates here) |
| `/app/data` | named volume | Sessions, plans, local config |
| `/app/data/.brainpro` | named volume | metrics.json, yo_history |
| `/app/logs` | named volume | Application logs |
| `/run` | tmpfs | Unix sockets (brainpro.sock) |
| `/var/run` | tmpfs | supervisor.sock |
| `/var/log/supervisor` | tmpfs | Supervisor logs |
| `/app/scratch` | tmpfs | Temporary validation files |

**Workspace Binding**: Mount your project directory to `/app/workspace`:

```yaml
volumes:
  - ./my-project:/app/workspace
```

The container's `working_dir` is `/app/workspace`, so all file operations happen relative to your mounted project.

**Secrets**: API keys are loaded via Docker secrets (12-factor pattern):

```yaml
secrets:
  anthropic_api_key:
    file: ./secrets/anthropic_api_key.txt

services:
  brainpro:
    secrets:
      - anthropic_api_key
```

The entrypoint exports secrets as environment variables - no file mutation required.

### Environment Variables

| Variable | Description |
|----------|-------------|
| `BRAINPRO_DATA_DIR` | Data directory (sessions, config) |
| `BRAINPRO_GATEWAY_TOKEN` | Required auth token |
| `VENICE_API_KEY` | Venice API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `ANTHROPIC_API_KEY` | Anthropic API key |

### docker-compose.yml

```yaml
services:
  brainpro:
    build: .
    ports:
      - "18789:18789"
    volumes:
      - brainpro-data:/app/data
      - ./project:/app/project:ro  # mount project read-only
    environment:
      - BRAINPRO_GATEWAY_TOKEN=your-secret-token
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
```

---

## Part 4: Security Hardening

### Authentication

Always set `BRAINPRO_GATEWAY_TOKEN` in production:
```bash
export BRAINPRO_GATEWAY_TOKEN=$(openssl rand -hex 32)
```

Clients must include token in WebSocket handshake.

### Network

- Bind gateway to localhost only, use reverse proxy for external access
- TLS termination via nginx/Caddy
- Never expose Unix socket to network

### Secrets Management

Use Docker secrets for API keys:
```yaml
secrets:
  anthropic_key:
    file: ./secrets/anthropic_key.txt

services:
  brainpro:
    secrets:
      - anthropic_key
    environment:
      - ANTHROPIC_API_KEY_FILE=/run/secrets/anthropic_key
```

### Permission Modes

Lock down with `default` mode:
```toml
[permissions]
mode = "default"  # require approval for writes
```

Never use `bypassPermissions` in production.

### Rule Patterns

Deny dangerous commands:
```toml
[permissions]
deny = [
  "Bash(rm -rf:*)",
  "Bash(curl:*)",
  "Bash(wget:*)",
  "Bash(nc:*)",
  "Bash(dd:*)",
]
```

Allow safe operations:
```toml
[permissions]
allow = [
  "Bash(git status)",
  "Bash(git diff:*)",
  "Bash(cargo test:*)",
]
```

### Built-in Protections

- `curl` and `wget` blocked by default
- All file paths validated to project root
- Symlinks resolved to prevent directory escape

### Audit Logging

Transcripts are written to `~/.brainpro/sessions/<uuid>.jsonl`.

For production:
- Rotate logs periodically
- Ship to central logging
- Monitor for denied tool calls

### Container Isolation

```yaml
services:
  brainpro:
    user: "1000:1000"  # non-root
    read_only: true
    volumes:
      - ./project:/app/project:ro  # read-only source
      - brainpro-data:/app/data    # writes only here
```

---

## Part 5: Advanced Configuration

### Full Config Reference

```toml
# Default LLM target
default_target = "qwen3-235b-a22b-instruct-2507@venice"

# Backend definitions (zdr = Zero Data Retention policy)
[backends.venice]
base_url = "https://api.venice.ai/api/v1"
api_key_env = "VENICE_API_KEY"
zdr = true

[backends.claude]
base_url = "https://api.anthropic.com/v1"
api_key_env = "ANTHROPIC_API_KEY"
zdr = true

[backends.chatgpt]
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
zdr = false  # OpenAI may train on data

[backends.ollama]
base_url = "http://localhost:11434/v1"
zdr = true   # Local = inherently ZDR
# No API key needed for local Ollama

# Permission rules
[permissions]
mode = "default"
allow = ["Bash(git:*)"]
ask = ["Write"]
deny = ["Bash(rm -rf:*)"]

# Bash tool settings
[bash]
timeout_ms = 120000
max_output_bytes = 200000

# Context management
[context]
max_chars = 250000
auto_compact_enabled = true

# Model routing
[model_routing.routes]
planning = "qwen3-235b-a22b-instruct-2507@venice"
coding = "claude-3-5-sonnet-latest@claude"
exploration = "gpt-4o-mini@chatgpt"

# MCP servers
[mcp.servers.calc]
command = "/path/to/mcp-calc"
transport = "stdio"
auto_start = false

# Circuit breaker (protects against cascading failures)
[circuit_breaker]
failure_threshold = 5      # Consecutive failures to open
recovery_timeout_secs = 30 # Seconds before half-open
half_open_probes = 3       # Successful probes to close
enabled = true

# Provider health tracking
[health]
degraded_latency_ms = 5000      # Latency threshold for degraded
unhealthy_failures = 3          # Consecutive failures for unhealthy
cooldown_secs = 60              # Cooldown period after unhealthy
latency_window_size = 10        # Sliding window for avg latency

# Fallback chains (automatic failover)
[fallback_chains]
primary = "claude-3-5-sonnet-latest@claude"
secondary = "gpt-4o@chatgpt"
local = "llama3:8b@ollama"
auto_local_fallback = true  # Fallback to Ollama when cloud exhausted

[fallback_chains.category_overrides.coding]
chain = ["claude@claude", "gpt-4o@chatgpt"]

# Privacy / Zero Data Retention
[privacy]
default_level = "sensitive"  # standard|sensitive|strict
audit_zdr_violations = true
prefer_local_for_sensitive = true
strict_patterns = ["password", "secret", "api_key", "token"]
```

### Model Routing

Auto-select models based on task type:

| Category | Keywords | Default |
|----------|----------|---------|
| `planning` | plan, architect, design | qwen3-235b@venice |
| `coding` | patch, edit, implement | claude-3-5-sonnet@claude |
| `exploration` | scout, find, search | gpt-4o-mini@chatgpt |
| `testing` | test, verify | gpt-4o-mini@chatgpt |
| `documentation` | doc, readme | gpt-4o-mini@chatgpt |

### Subagent Creation

Create `.brainpro/agents/scout.toml`:
```toml
name = "scout"
description = "Read-only exploration"
allowed_tools = ["Read", "Grep", "Glob"]
permission_mode = "default"
max_turns = 8
system_prompt = """
You are Scout, a read-only exploration agent.
Find files, search content, examine code.
"""
# Optional: override target
# target = "gpt-4o-mini@chatgpt"
```

Use via REPL:
```
/task scout find where config is parsed
```

### MCP Server Integration

```toml
[mcp.servers.database]
command = "/usr/local/bin/mcp-postgres"
transport = "stdio"
auto_start = true

[mcp.servers.external]
url = "https://mcp.example.com/v1"
transport = "http"
```

Commands:
```
/mcp list
/mcp connect database
/mcp tools database
```

### Custom Slash Commands

Create `.brainpro/commands/fix-issue.md`:
```markdown
---
description: Fix GitHub issue by number
allowed_tools:
  - Read
  - Edit
  - Bash
---
Fix issue #$ARGUMENTS.
1. Read the issue details
2. Find relevant code
3. Make the fix
4. Write a test
```

Use: `/fix-issue 123`

---

## Part 6: Examples & Recipes

### Multi-Model Workflow

Plan with Qwen, implement with Claude:
```bash
yo --target qwen3-235b@venice -p "design a caching layer for the API"
# Review plan, then:
yo --target claude-3-5-sonnet@claude -p "implement the caching layer per the plan"
```

### Read-Only Exploration Agent

`.brainpro/agents/readonly.toml`:
```toml
name = "readonly"
description = "Safe exploration, no writes"
allowed_tools = ["Read", "Grep", "Glob"]
permission_mode = "default"
system_prompt = "Explore and explain code. Never suggest edits."
```

### CI/CD Integration

```yaml
# .github/workflows/review.yml
jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release
      - run: |
          ./target/release/yo \
            --mode default \
            -p "review the changes in this PR for bugs and security issues" \
            > review.md
      - uses: actions/upload-artifact@v4
        with:
          name: review
          path: review.md
```

### Custom Skill Pack

`.brainpro/skills/secure-review/SKILL.md`:
```markdown
---
name: secure-review
description: Security-focused code review
allowed-tools: Read, Grep, Glob
---
You are a security reviewer. Focus on:
- Input validation
- SQL injection
- XSS vulnerabilities
- Authentication flaws
- Secrets in code

Never modify files. Only report findings.
```

Activate: `/skillpack use secure-review`

---

## Part 7: Resilience & Privacy

### Circuit Breaker

Protects against cascading failures when backends are unhealthy.

**States**: Closed → Open → HalfOpen → Closed

- **Closed**: Normal operation, requests pass through
- **Open**: Failures exceeded threshold, requests rejected immediately
- **HalfOpen**: Recovery period, limited probe requests

```toml
[circuit_breaker]
failure_threshold = 5      # Consecutive failures to trip open
recovery_timeout_secs = 30 # Time before attempting recovery
half_open_probes = 3       # Successful probes needed to close
enabled = true
```

### Provider Health Tracking

Monitors backend health for intelligent routing decisions.

**Health States**:
- `Healthy`: Normal latency, no failures
- `Degraded`: High latency or intermittent failures
- `Unhealthy`: Consistent failures, in cooldown

```toml
[health]
degraded_latency_ms = 5000  # Latency threshold for degraded state
unhealthy_failures = 3      # Failures before marking unhealthy
cooldown_secs = 60          # Cooldown before retry
latency_window_size = 10    # Sliding window for latency calculation
```

### Fallback Chains

Automatic failover when primary providers fail.

```toml
[fallback_chains]
primary = "claude-3-5-sonnet-latest@claude"
secondary = "gpt-4o@chatgpt"
local = "llama3:8b@ollama"
auto_local_fallback = true  # Fallback to Ollama when cloud exhausted

# Override for specific task categories
[fallback_chains.category_overrides.coding]
chain = ["claude@claude", "gpt-4o@chatgpt"]
```

Fallback triggers:
- Circuit breaker open
- Provider unhealthy
- Rate limited (429)
- Server error (5xx)

### Zero Data Retention (ZDR)

Privacy-first routing ensures sensitive data stays with trusted providers.

**Privacy Levels**:
| Level | Behavior |
|-------|----------|
| `standard` | Any provider acceptable |
| `sensitive` | Prefer ZDR providers, warn if non-ZDR used |
| `strict` | ZDR-only providers, fail if unavailable |

**Configuration**:
```toml
[privacy]
default_level = "sensitive"
audit_zdr_violations = true
prefer_local_for_sensitive = true
strict_patterns = ["password", "secret", "api_key", "token", "ssn"]
```

**Backend ZDR flags**:
```toml
[backends.venice]
zdr = true   # Venice has ZDR policy

[backends.claude]
zdr = true   # Anthropic has ZDR option

[backends.chatgpt]
zdr = false  # OpenAI may train on data

[backends.ollama]
zdr = true   # Local = inherently ZDR
```

**Auto-escalation**: Prompts containing `strict_patterns` auto-escalate to `strict` level.

### Credential Security

- API keys stored via environment variables (preferred) or config
- Keys wrapped in `SecretString` (zeroized on drop)
- Credentials never logged or displayed
- TLS 1.2+ enforced for all connections

Best practice:
```bash
# Use environment variables, not config files
export ANTHROPIC_API_KEY="sk-..."
export VENICE_API_KEY="..."
```

### Observability

Prometheus metrics available at `/metrics` (gateway mode):
- `brainpro_requests_total{backend,model,status}`
- `brainpro_requests_duration_ms{backend,model}`
- `brainpro_circuit_trips_total{backend}`
- `brainpro_tokens_total{backend,model,direction}`
- `brainpro_cost_usd_total{backend,model}`

JSON export: `~/.brainpro/metrics.json`
