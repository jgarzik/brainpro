//! Plan mode: structured planning before execution.
//!
//! Plan mode allows the LLM to explore the codebase with read-only tools
//! and produce a structured implementation plan before executing changes.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Cached regex for parsing STEP lines in plan output
static STEP_REGEX: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"(?i)STEP\s+(\d+):\s*(.+)").expect("Invalid step regex"));

// ============================================================================
// Core Data Structures
// ============================================================================

/// Status of the overall plan
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    #[default]
    Draft,
    Ready,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

impl PlanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ready => "ready",
            Self::Executing => "executing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Status of a single plan step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl PlanStepStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Pending => "[ ]",
            Self::InProgress => "[>]",
            Self::Completed => "[x]",
            Self::Failed => "[!]",
            Self::Skipped => "[-]",
        }
    }
}

/// A single step in an implementation plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Step number (1-indexed)
    pub number: usize,
    /// Brief title for the step
    pub title: String,
    /// Detailed description of what to do
    pub description: String,
    /// Files to be read or modified
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    /// Tools expected to be used (e.g., "Edit", "Write", "Bash")
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    /// Current status
    #[serde(default)]
    pub status: PlanStepStatus,
    /// Optional output/notes from execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

impl PlanStep {
    pub fn new(number: usize, title: String, description: String) -> Self {
        Self {
            number,
            title,
            description,
            files: Vec::new(),
            tools: Vec::new(),
            status: PlanStepStatus::Pending,
            output: None,
        }
    }
}

/// Context gathered during planning phase
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanContext {
    /// Files that were read during planning
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_read: Vec<String>,
    /// Key findings from codebase exploration
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

/// A complete implementation plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Plan name/identifier
    pub name: String,
    /// Original task/goal description
    pub goal: String,
    /// High-level summary of the approach
    #[serde(default)]
    pub summary: String,
    /// Ordered list of steps
    #[serde(default)]
    pub steps: Vec<PlanStep>,
    /// When the plan was created
    pub created_at: DateTime<Utc>,
    /// When the plan was last modified
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<DateTime<Utc>>,
    /// Overall plan status
    #[serde(default)]
    pub status: PlanStatus,
    /// Context gathered during planning
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<PlanContext>,
}

impl Plan {
    pub fn new(name: String, goal: String) -> Self {
        Self {
            name,
            goal,
            summary: String::new(),
            steps: Vec::new(),
            created_at: Utc::now(),
            modified_at: None,
            status: PlanStatus::Draft,
            context: Some(PlanContext::default()),
        }
    }

    /// Get the next pending step
    pub fn next_step(&self) -> Option<&PlanStep> {
        self.steps
            .iter()
            .find(|s| s.status == PlanStepStatus::Pending)
    }

    /// Get a mutable reference to a step by number
    pub fn step_mut(&mut self, number: usize) -> Option<&mut PlanStep> {
        self.steps.iter_mut().find(|s| s.number == number)
    }

    /// Count completed steps
    pub fn completed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == PlanStepStatus::Completed)
            .count()
    }

    /// Count failed steps
    pub fn failed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == PlanStepStatus::Failed)
            .count()
    }

    /// Format plan for display
    pub fn format_display(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Plan: {}\n\n", self.name));
        out.push_str(&format!("**Goal:** {}\n\n", self.goal));
        if !self.summary.is_empty() {
            out.push_str(&format!("**Summary:** {}\n\n", self.summary));
        }
        out.push_str(&format!("**Status:** {}\n\n", self.status.as_str()));
        out.push_str("## Steps\n\n");

        for step in &self.steps {
            out.push_str(&format!(
                "{} **Step {}:** {}\n",
                step.status.icon(),
                step.number,
                step.title
            ));
            out.push_str(&format!("   {}\n", step.description));
            if !step.files.is_empty() {
                out.push_str(&format!("   Files: {}\n", step.files.join(", ")));
            }
            if !step.tools.is_empty() {
                out.push_str(&format!("   Tools: {}\n", step.tools.join(", ")));
            }
            out.push('\n');
        }

        out
    }
}

// ============================================================================
// Plan Mode State
// ============================================================================

/// Phase within plan mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlanPhase {
    #[default]
    Inactive,
    /// Gathering requirements and exploring codebase
    Planning,
    /// Plan is ready for review
    Review,
    /// Executing plan steps
    Executing,
}

/// State for plan mode in the REPL
#[derive(Debug, Default)]
pub struct PlanModeState {
    /// Whether plan mode is currently active
    pub active: bool,
    /// Current plan being built or reviewed
    pub current_plan: Option<Plan>,
    /// Phase within plan mode
    pub phase: PlanPhase,
}

impl PlanModeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter planning mode with a goal
    pub fn enter_planning(&mut self, goal: String) {
        let name = Self::generate_plan_name(&goal);
        self.active = true;
        self.current_plan = Some(Plan::new(name, goal));
        self.phase = PlanPhase::Planning;
    }

    /// Transition to review phase
    pub fn enter_review(&mut self) {
        self.phase = PlanPhase::Review;
        if let Some(plan) = &mut self.current_plan {
            plan.status = PlanStatus::Ready;
        }
    }

    /// Transition to executing phase
    pub fn enter_executing(&mut self) {
        self.phase = PlanPhase::Executing;
        if let Some(plan) = &mut self.current_plan {
            plan.status = PlanStatus::Executing;
        }
    }

    /// Exit plan mode entirely
    pub fn exit(&mut self) {
        self.active = false;
        self.current_plan = None;
        self.phase = PlanPhase::Inactive;
    }

    /// Load a plan into review mode
    pub fn load_plan(&mut self, plan: Plan) {
        self.active = true;
        self.current_plan = Some(plan);
        self.phase = PlanPhase::Review;
    }

    /// Generate a plan name from the goal
    fn generate_plan_name(goal: &str) -> String {
        let slug: String = goal
            .to_lowercase()
            .chars()
            .take(30)
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect();
        let slug = slug.trim_matches('-');
        format!("{}-{}", slug, Utc::now().format("%Y%m%d-%H%M%S"))
    }
}

// ============================================================================
// Persistence
// ============================================================================

/// Metadata for listing plans without loading full content
#[derive(Debug)]
pub struct PlanMetadata {
    pub name: String,
    pub goal: String,
    pub status: PlanStatus,
    pub created_at: DateTime<Utc>,
    pub step_count: usize,
}

/// Get the plans directory path
pub fn plans_dir(root: &Path) -> PathBuf {
    root.join(".yo").join("plans")
}

/// Save a plan to disk
pub fn save_plan(plan: &Plan, root: &Path) -> Result<PathBuf> {
    let dir = plans_dir(root);
    std::fs::create_dir_all(&dir)?;

    let filename = format!("{}.toml", plan.name);
    let path = dir.join(&filename);

    let content = toml::to_string_pretty(plan)?;
    std::fs::write(&path, content)?;

    Ok(path)
}

/// Load a plan from disk by name
pub fn load_plan(name: &str, root: &Path) -> Result<Plan> {
    let path = plans_dir(root).join(format!("{}.toml", name));
    let content =
        std::fs::read_to_string(&path).map_err(|_| anyhow!("Plan not found: {}", name))?;
    let plan: Plan = toml::from_str(&content)?;
    Ok(plan)
}

/// List all saved plans
pub fn list_plans(root: &Path) -> Result<Vec<PlanMetadata>> {
    let dir = plans_dir(root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut plans = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(plan) = toml::from_str::<Plan>(&content) {
                    plans.push(PlanMetadata {
                        name: plan.name,
                        goal: plan.goal,
                        status: plan.status,
                        created_at: plan.created_at,
                        step_count: plan.steps.len(),
                    });
                }
            }
        }
    }

    // Sort by creation date, newest first
    plans.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(plans)
}

/// Delete a saved plan
pub fn delete_plan(name: &str, root: &Path) -> Result<()> {
    let path = plans_dir(root).join(format!("{}.toml", name));
    std::fs::remove_file(&path).map_err(|_| anyhow!("Plan not found: {}", name))?;
    Ok(())
}

// ============================================================================
// Plan Parsing
// ============================================================================

/// Parse a plan from LLM output
pub fn parse_plan_output(output: &str, goal: &str) -> Result<Plan> {
    let mut plan = Plan::new(PlanModeState::generate_plan_name(goal), goal.to_string());

    // Find the ```plan block
    let plan_block = extract_plan_block(output)?;

    // Parse SUMMARY
    if let Some(summary) = extract_field(&plan_block, "SUMMARY:") {
        plan.summary = summary;
    }

    // Parse STEPs using cached regex
    let mut current_step: Option<PlanStep> = None;
    let mut in_description = false;

    for line in plan_block.lines() {
        let trimmed = line.trim();

        if let Some(caps) = STEP_REGEX.captures(trimmed) {
            // Save previous step if exists
            if let Some(step) = current_step.take() {
                plan.steps.push(step);
            }

            let number: usize = caps[1].parse()?;
            let title = caps[2].trim().to_string();
            current_step = Some(PlanStep::new(number, title, String::new()));
            in_description = false;
        } else if let Some(ref mut step) = current_step {
            // Parse step fields
            if let Some(desc) = trimmed.strip_prefix("DESCRIPTION:") {
                step.description = desc.trim().to_string();
                in_description = true;
            } else if let Some(files) = trimmed.strip_prefix("FILES:") {
                step.files = files
                    .split(',')
                    .map(|f| f.trim().to_string())
                    .filter(|f| !f.is_empty())
                    .collect();
                in_description = false;
            } else if let Some(tools) = trimmed.strip_prefix("TOOLS:") {
                step.tools = tools
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                in_description = false;
            } else if in_description && !trimmed.is_empty() {
                // Continue description on next line
                if !step.description.is_empty() {
                    step.description.push(' ');
                }
                step.description.push_str(trimmed);
            }
        }
    }

    // Don't forget the last step
    if let Some(step) = current_step {
        plan.steps.push(step);
    }

    if plan.steps.is_empty() {
        return Err(anyhow!("No steps found in plan output"));
    }

    plan.status = PlanStatus::Ready;
    Ok(plan)
}

fn extract_plan_block(output: &str) -> Result<String> {
    // Look for ```plan block
    if let Some(start) = output.find("```plan") {
        let content_start = start + 7;
        let end = output[content_start..]
            .find("```")
            .map(|i| content_start + i)
            .unwrap_or(output.len());
        return Ok(output[content_start..end].trim().to_string());
    }

    // Fallback: look for STEP markers directly
    if output.contains("STEP 1:") || output.contains("Step 1:") {
        return Ok(output.to_string());
    }

    Err(anyhow!(
        "No plan block found. Expected ```plan...``` or STEP markers"
    ))
}

fn extract_field(content: &str, prefix: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(value) = line.trim().strip_prefix(prefix) {
            return Some(value.trim().to_string());
        }
    }
    None
}

// ============================================================================
// System Prompts
// ============================================================================

pub const PLAN_MODE_SYSTEM_PROMPT: &str = r#"You are in PLAN MODE. Create a detailed, executable implementation plan.

## Available Tools
You have READ-ONLY access: Read, Grep, Glob
Use these to explore the codebase and understand existing patterns.

## Plan Structure

Start with a header:

# [Feature Name] Implementation Plan

**Goal:** [Single sentence describing the objective]
**Architecture:** [2-3 sentences explaining the technical approach]
**Tech Stack:** [Key technologies, libraries, patterns involved]

Then provide steps. Each step MUST include:

## STEP N: [Descriptive Title]

**Files:**
- CREATE: path/to/new/file.ext (for new files)
- MODIFY: path/to/existing/file.ext (for changes)

**Implementation:**
```language
// Complete, copy-pasteable code
// Include the actual code to add or change
```

**Verification:**
```bash
command to run
# Expected: description of success
```

## Guidelines

1. **Explore First** - Use Glob/Grep/Read to understand existing code patterns before planning
2. **Complete Code** - Include actual, runnable code - not descriptions or pseudocode
3. **Exact Verification** - Specify commands to verify each step works
4. **Atomic Steps** - Each step should be independently verifiable
5. **Logical Order** - Steps should build on each other appropriately
6. **Match Patterns** - Follow existing code style and patterns found in the codebase

DO NOT execute changes. Only produce the plan."#;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_step_creation() {
        let step = PlanStep::new(1, "Test step".to_string(), "Do something".to_string());
        assert_eq!(step.number, 1);
        assert_eq!(step.title, "Test step");
        assert_eq!(step.status, PlanStepStatus::Pending);
    }

    #[test]
    fn test_plan_creation() {
        let plan = Plan::new("test-plan".to_string(), "Test goal".to_string());
        assert_eq!(plan.name, "test-plan");
        assert_eq!(plan.goal, "Test goal");
        assert_eq!(plan.status, PlanStatus::Draft);
        assert!(plan.steps.is_empty());
    }

    #[test]
    fn test_plan_mode_state() {
        let mut state = PlanModeState::new();
        assert!(!state.active);
        assert_eq!(state.phase, PlanPhase::Inactive);

        state.enter_planning("Add feature".to_string());
        assert!(state.active);
        assert_eq!(state.phase, PlanPhase::Planning);
        assert!(state.current_plan.is_some());

        state.enter_review();
        assert_eq!(state.phase, PlanPhase::Review);

        state.exit();
        assert!(!state.active);
        assert_eq!(state.phase, PlanPhase::Inactive);
    }

    #[test]
    fn test_parse_plan_output() {
        let output = r#"
Let me explore the codebase first...

```plan
SUMMARY: Add a new feature to the system

STEP 1: Create the module
DESCRIPTION: Create a new module file with basic structure
FILES: src/feature.rs
TOOLS: Write

STEP 2: Add tests
DESCRIPTION: Write unit tests for the module
FILES: src/feature.rs
TOOLS: Edit
```

That's the plan!
"#;

        let plan = parse_plan_output(output, "Add feature").unwrap();
        assert_eq!(plan.summary, "Add a new feature to the system");
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].title, "Create the module");
        assert_eq!(plan.steps[0].files, vec!["src/feature.rs"]);
        assert_eq!(plan.steps[1].number, 2);
    }

    #[test]
    fn test_parse_plan_without_block() {
        let output = r#"
SUMMARY: Simple plan

STEP 1: Do thing
DESCRIPTION: Do the thing
FILES: file.rs
TOOLS: Edit
"#;

        let plan = parse_plan_output(output, "Simple task").unwrap();
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn test_plan_display() {
        let mut plan = Plan::new("test".to_string(), "Test goal".to_string());
        plan.summary = "Test summary".to_string();
        plan.steps.push(PlanStep::new(
            1,
            "Step one".to_string(),
            "Do step one".to_string(),
        ));

        let display = plan.format_display();
        assert!(display.contains("# Plan: test"));
        assert!(display.contains("**Goal:** Test goal"));
        assert!(display.contains("Step 1:"));
    }
}
