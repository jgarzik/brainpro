# Future Optimization Ideas for `-O` Mode

This document captures ideas for future enhancements to the `-O` optimization flag, building on the research that shorter, denser prompts improve LLM performance.

## Research Foundation

- **Context Rot**: Accuracy degrades as prompts grow longer (Chroma study on 18 models)
- **LLMLingua**: 20x compression with only 1.5% performance loss
- **Positive Framing**: "Do this" outperforms "don't do this" in prompts
- **Signal Density**: "Find the smallest set of high-signal tokens that maximize the likelihood of your desired outcome"

Sources:
- https://github.com/microsoft/LLMLingua
- https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- https://gritdaily.com/impact-prompt-length-llm-performance/

---

## Implemented Layers

### Layer 1: Terse System Prompt
- Reduced from ~60 tokens to ~15 tokens
- Positive framing: "AI-to-AI mode. Maximum information density. Structure over prose. No narration."

### Layer 2: Compressed Tool Schemas
- Tool descriptions shortened (e.g., "Read file content. Paths relative to project root." → "Read file")
- Parameter descriptions stripped in optimize mode
- Uses `SchemaOptions` struct for extensibility

---

## Future Layers

### Layer 3: Tool Result Compression

**Concept**: Strip metadata from tool results in `-O` mode.

Current Read result:
```json
{
  "path": "foo.rs",
  "offset": 0,
  "truncated": false,
  "content": "...",
  "sha256": "abc123",
  "lines": 42
}
```

Optimized result:
```json
{"content": "..."}
```

**Implementation**:
- Add `optimize` flag to `tools::execute()`
- Conditionally strip fields: `path`, `offset`, `truncated`, `sha256`, `lines`
- Keep only essential data needed for task completion

**Estimated token savings**: 30-50% per tool result

---

### Layer 4: History Summarization ✓ (Partial)

**Status**: Basic implementation in `compact.rs`. Uses LLM to summarize older messages when context grows large. Triggered via `/compact` command or auto-compact threshold.

**Concept**: Compress older conversation turns to maintain context while reducing tokens.

**Approaches**:
1. **Sliding Window**: Keep only last N turns in full, summarize older ones
2. **Semantic Compression**: Use small model to compress verbose assistant responses
3. **Result Deduplication**: Merge repeated tool results (e.g., multiple Read calls on same file)

**Implementation ideas**:
- Add `conversation_compressor` module
- Trigger compression when context exceeds threshold
- Preserve tool call/result structure for agent continuity

**Research reference**: LLMLingua-2 achieves 3-6x faster compression with task-agnostic distillation

---

### Layer 5: Output Style Enforcement

**Concept**: Enforce structured output format in `-O` mode.

**Current**: LLM outputs natural language explanations mixed with actions
**Optimized**: Pure structured output, no prose

**Implementation ideas**:
1. **Structured Output Schema**: Add JSON schema for responses
2. **Response Format Instruction**: "Respond only with tool calls or structured JSON"
3. **Post-processing**: Strip explanation text, keep only actions

**Example transformation**:
```
Before: "I'll read the config file to understand the settings. Let me use the Read tool..."
After:  [tool_call: Read, path: "config.toml"]
```

**Trade-off**: May reduce transparency for human review, but ideal for AI-to-AI pipelines

---

### Layer 6: Dynamic Tool Injection

**Concept**: Only include tool schemas likely needed for the current task.

**Current**: All 8 tools included in every request
**Optimized**: Analyze prompt, inject relevant subset

**Heuristics**:
- "read", "view", "show" → Read, Grep, Glob
- "edit", "modify", "change" → Read, Edit, Write
- "run", "execute", "test", "build" → Bash
- "find", "search" → Grep, Glob
- "delegate", "subagent" → Task

**Implementation**:
- Add `infer_tools_from_prompt(prompt: &str) -> Vec<ToolName>`
- Apply before schema generation
- Fall back to full toolset if uncertain

---

### Layer 7: CodeAgents-Style Pseudocode

**Concept**: Use structured pseudocode instead of natural language for reasoning.

**Research**: CodeAgents framework reduces tokens by 55-87%.

**Current**:
```
I need to first read the file to understand its structure, then I'll make the edit...
```

**Optimized**:
```
PLAN: Read("src/main.rs") -> Edit(find="old", replace="new")
```

**Implementation**:
- Add `--reasoning-format=pseudocode` option
- Train/prompt model to use structured planning notation
- Parse pseudocode for execution

---

## Measurement & Validation

To validate optimization effectiveness:

1. **Token Counting**: Compare input/output tokens with and without `-O`
2. **Task Success Rate**: Ensure optimizations don't reduce accuracy
3. **Latency**: Measure time-to-first-token improvement
4. **Cost**: Calculate API cost savings

**Suggested benchmarks**:
- Simple file read/edit tasks
- Multi-step refactoring tasks
- Codebase exploration tasks

---

## Configuration Ideas

Future `SchemaOptions` extensions:
```rust
pub struct SchemaOptions {
    pub optimize: bool,
    // Future fields:
    pub compress_results: bool,
    pub dynamic_tools: bool,
    pub pseudocode_reasoning: bool,
    pub max_history_turns: Option<usize>,
}
```

Command-line exposure:
```
yo -O                    # Enable all optimizations
yo -O --no-compress      # Optimize schemas but not results
yo --optimize-level=2    # Granular control
```
