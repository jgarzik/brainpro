//! Model routing for situational model selection.
//!
//! This module enables automatic model selection based on:
//! - Subagent type (inferred from name/description)
//! - Provider health (skip unhealthy backends)
//! - Cost awareness (prefer cheaper for simple tasks, respect budget)
//! - Context requirements (route to models with sufficient context window)
//! - Privacy level (route to ZDR providers for sensitive data)
//! - Explicit @model annotations in prompts
//! - Fallback chains when primary providers fail

#![allow(dead_code)]

use crate::config::Target;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Route categories for model selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteCategory {
    Planning,
    Coding,
    Exploration,
    Testing,
    Documentation,
    Fast,
    Default,
}

impl RouteCategory {
    /// Infer category from agent name/description
    pub fn from_agent_name(name: &str, description: &str) -> Self {
        let combined = format!("{} {}", name, description).to_lowercase();

        if combined.contains("plan")
            || combined.contains("architect")
            || combined.contains("design")
        {
            RouteCategory::Planning
        } else if combined.contains("patch")
            || combined.contains("edit")
            || combined.contains("refactor")
            || combined.contains("code")
            || combined.contains("implement")
        {
            RouteCategory::Coding
        } else if combined.contains("scout")
            || combined.contains("explore")
            || combined.contains("find")
            || combined.contains("search")
        {
            RouteCategory::Exploration
        } else if combined.contains("test")
            || combined.contains("verify")
            || combined.contains("check")
        {
            RouteCategory::Testing
        } else if combined.contains("doc")
            || combined.contains("readme")
            || combined.contains("comment")
        {
            RouteCategory::Documentation
        } else {
            RouteCategory::Default
        }
    }

    /// Get the category name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            RouteCategory::Planning => "planning",
            RouteCategory::Coding => "coding",
            RouteCategory::Exploration => "exploration",
            RouteCategory::Testing => "testing",
            RouteCategory::Documentation => "documentation",
            RouteCategory::Fast => "fast",
            RouteCategory::Default => "default",
        }
    }
}

/// Cost tier for models
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CostTier {
    Low,
    #[default]
    Medium,
    High,
    Premium,
}

/// Capabilities of a specific model
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelCapabilities {
    /// Maximum context window in tokens
    #[serde(default = "default_context_window")]
    pub context_window: usize,
    /// Cost tier
    #[serde(default)]
    pub cost_tier: CostTier,
    /// Whether this model supports tool/function calling
    #[serde(default = "default_supports_tools")]
    pub supports_tools: bool,
    /// Whether the backend has ZDR
    #[serde(default)]
    pub zdr: bool,
}

fn default_context_window() -> usize {
    128000 // 128k tokens default
}

fn default_supports_tools() -> bool {
    true
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            context_window: default_context_window(),
            cost_tier: CostTier::Medium,
            supports_tools: true,
            zdr: false,
        }
    }
}

/// Known model capabilities (hardcoded defaults)
fn default_model_capabilities() -> HashMap<String, ModelCapabilities> {
    let mut caps = HashMap::new();

    // Claude models
    caps.insert(
        "claude-3-5-sonnet-latest".to_string(),
        ModelCapabilities {
            context_window: 200000,
            cost_tier: CostTier::Medium,
            supports_tools: true,
            zdr: true,
        },
    );
    caps.insert(
        "claude-3-opus".to_string(),
        ModelCapabilities {
            context_window: 200000,
            cost_tier: CostTier::Premium,
            supports_tools: true,
            zdr: true,
        },
    );

    // OpenAI models
    caps.insert(
        "gpt-4o".to_string(),
        ModelCapabilities {
            context_window: 128000,
            cost_tier: CostTier::High,
            supports_tools: true,
            zdr: false,
        },
    );
    caps.insert(
        "gpt-4o-mini".to_string(),
        ModelCapabilities {
            context_window: 128000,
            cost_tier: CostTier::Low,
            supports_tools: true,
            zdr: false,
        },
    );

    // Venice models
    caps.insert(
        "claude-sonnet-45".to_string(),
        ModelCapabilities {
            context_window: 200000,
            cost_tier: CostTier::Medium,
            supports_tools: true,
            zdr: true,
        },
    );
    caps.insert(
        "qwen3-235b-a22b-instruct-2507".to_string(),
        ModelCapabilities {
            context_window: 131072,
            cost_tier: CostTier::Medium,
            supports_tools: true,
            zdr: true,
        },
    );
    caps.insert(
        "llama-3.3-70b".to_string(),
        ModelCapabilities {
            context_window: 128000,
            cost_tier: CostTier::Low,
            supports_tools: true,
            zdr: true,
        },
    );

    // Ollama/local models (assume ZDR since local)
    caps.insert(
        "llama3:8b".to_string(),
        ModelCapabilities {
            context_window: 8192,
            cost_tier: CostTier::Low,
            supports_tools: true,
            zdr: true,
        },
    );

    caps
}

/// Configuration for model routing
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModelRoutingConfig {
    #[serde(default)]
    pub routes: HashMap<RouteCategory, String>, // category -> target string
    #[serde(default)]
    pub model_caps: HashMap<String, ModelCapabilities>,
}

/// Hardcoded default routes (sensible defaults)
fn default_routes() -> HashMap<RouteCategory, String> {
    let mut routes = HashMap::new();

    // These are sensible defaults - users can override in config
    // Format: model@backend

    // Planning tasks benefit from strong reasoning
    routes.insert(
        RouteCategory::Planning,
        "claude-sonnet-45@venice".to_string(),
    );

    // Coding needs strong code generation
    routes.insert(
        RouteCategory::Coding,
        "claude-3-5-sonnet-latest@claude".to_string(),
    );

    // Exploration can use faster models
    routes.insert(
        RouteCategory::Exploration,
        "gpt-4o-mini@chatgpt".to_string(),
    );

    // Testing needs reliable execution
    routes.insert(RouteCategory::Testing, "gpt-4o-mini@chatgpt".to_string());

    // Documentation
    routes.insert(
        RouteCategory::Documentation,
        "gpt-4o-mini@chatgpt".to_string(),
    );

    // Fast operations
    routes.insert(RouteCategory::Fast, "gpt-4o-mini@chatgpt".to_string());

    // Default fallback
    routes.insert(RouteCategory::Default, "gpt-4o-mini@chatgpt".to_string());

    routes
}

/// Context for routing decisions
#[derive(Debug, Clone, Default)]
pub struct RoutingContext {
    /// Estimated tokens needed for this request
    pub estimated_tokens: Option<usize>,
    /// Whether this request requires ZDR
    pub require_zdr: bool,
    /// Maximum cost tier allowed
    pub max_cost_tier: Option<CostTier>,
    /// Whether tools are required
    pub require_tools: bool,
    /// Explicit @model annotation from prompt
    pub explicit_model: Option<String>,
    /// Backends that are currently unavailable
    pub unavailable_backends: Vec<String>,
}

impl RoutingContext {
    /// Parse @model annotation from prompt text
    pub fn extract_model_annotation(prompt: &str) -> Option<String> {
        // Look for @model_name@backend or @model_name patterns
        for word in prompt.split_whitespace() {
            if word.starts_with('@') && word.len() > 1 {
                let annotation = &word[1..];
                // Skip common @ mentions that aren't model specs
                if annotation.contains('@')
                    || annotation.chars().all(|c| {
                        c.is_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '.'
                    })
                {
                    return Some(annotation.to_string());
                }
            }
        }
        None
    }
}

/// Model router that resolves targets based on context
pub struct ModelRouter {
    config: ModelRoutingConfig,
    defaults: HashMap<RouteCategory, String>,
    model_caps: HashMap<String, ModelCapabilities>,
}

impl ModelRouter {
    pub fn new(config: ModelRoutingConfig) -> Self {
        let mut model_caps = default_model_capabilities();
        // Merge user-defined capabilities
        for (model, caps) in &config.model_caps {
            model_caps.insert(model.clone(), caps.clone());
        }

        Self {
            config,
            defaults: default_routes(),
            model_caps,
        }
    }

    /// Get capabilities for a model
    pub fn get_capabilities(&self, model: &str) -> ModelCapabilities {
        self.model_caps.get(model).cloned().unwrap_or_default()
    }

    /// Check if a target meets routing context requirements
    fn meets_requirements(&self, target: &Target, ctx: &RoutingContext) -> bool {
        // Check if backend is available
        if ctx.unavailable_backends.contains(&target.backend) {
            return false;
        }

        let caps = self.get_capabilities(&target.model);

        // Check context window
        if let Some(needed) = ctx.estimated_tokens {
            if caps.context_window < needed {
                return false;
            }
        }

        // Check ZDR requirement
        if ctx.require_zdr && !caps.zdr {
            return false;
        }

        // Check cost tier
        if let Some(max_tier) = ctx.max_cost_tier {
            let tier_order = |t: CostTier| match t {
                CostTier::Low => 0,
                CostTier::Medium => 1,
                CostTier::High => 2,
                CostTier::Premium => 3,
            };
            if tier_order(caps.cost_tier) > tier_order(max_tier) {
                return false;
            }
        }

        // Check tool support
        if ctx.require_tools && !caps.supports_tools {
            return false;
        }

        true
    }

    /// Resolve target for a route category with context
    pub fn resolve_with_context(
        &self,
        category: RouteCategory,
        ctx: &RoutingContext,
        fallback: &Target,
    ) -> Target {
        // Check for explicit model annotation first
        if let Some(ref annotation) = ctx.explicit_model {
            if let Some(target) = Target::parse(annotation) {
                if self.meets_requirements(&target, ctx) {
                    return target;
                }
            }
        }

        // Check user config
        if let Some(target_str) = self.config.routes.get(&category) {
            if let Some(target) = Target::parse(target_str) {
                if self.meets_requirements(&target, ctx) {
                    return target;
                }
            }
        }

        // Check defaults
        if let Some(target_str) = self.defaults.get(&category) {
            if let Some(target) = Target::parse(target_str) {
                if self.meets_requirements(&target, ctx) {
                    return target;
                }
            }
        }

        // Fallback if it meets requirements
        if self.meets_requirements(fallback, ctx) {
            return fallback.clone();
        }

        // Last resort: return fallback even if it doesn't meet all requirements
        fallback.clone()
    }

    /// Resolve target for a route category (simple version without context)
    pub fn resolve(&self, category: RouteCategory, fallback: &Target) -> Target {
        self.resolve_with_context(category, &RoutingContext::default(), fallback)
    }

    /// Resolve target for an agent spec
    pub fn resolve_for_agent(
        &self,
        agent_name: &str,
        agent_description: &str,
        explicit_target: Option<&str>,
        fallback: &Target,
    ) -> Target {
        // Explicit target takes priority
        if let Some(target_str) = explicit_target {
            if let Some(target) = Target::parse(target_str) {
                return target;
            }
        }

        // Infer category and route
        let category = RouteCategory::from_agent_name(agent_name, agent_description);
        self.resolve(category, fallback)
    }

    /// Resolve target for an agent spec with full routing context
    pub fn resolve_for_agent_with_context(
        &self,
        agent_name: &str,
        agent_description: &str,
        explicit_target: Option<&str>,
        ctx: &RoutingContext,
        fallback: &Target,
    ) -> Target {
        // Explicit target takes priority if it meets requirements
        if let Some(target_str) = explicit_target {
            if let Some(target) = Target::parse(target_str) {
                if self.meets_requirements(&target, ctx) {
                    return target;
                }
            }
        }

        // Infer category and route with context
        let category = RouteCategory::from_agent_name(agent_name, agent_description);
        self.resolve_with_context(category, ctx, fallback)
    }

    /// Filter targets by availability and requirements
    pub fn filter_available(&self, targets: &[Target], ctx: &RoutingContext) -> Vec<Target> {
        targets
            .iter()
            .filter(|t| self.meets_requirements(t, ctx))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_inference() {
        assert_eq!(
            RouteCategory::from_agent_name("planner", "Plan the architecture"),
            RouteCategory::Planning
        );
        assert_eq!(
            RouteCategory::from_agent_name("patch", "Apply code edits"),
            RouteCategory::Coding
        );
        assert_eq!(
            RouteCategory::from_agent_name("scout", "Find files"),
            RouteCategory::Exploration
        );
        assert_eq!(
            RouteCategory::from_agent_name("test-runner", "Run tests"),
            RouteCategory::Testing
        );
        assert_eq!(
            RouteCategory::from_agent_name("docs", "Write documentation"),
            RouteCategory::Documentation
        );
        assert_eq!(
            RouteCategory::from_agent_name("unknown", "Some agent"),
            RouteCategory::Default
        );
    }

    #[test]
    fn test_router_explicit_target_priority() {
        let router = ModelRouter::new(ModelRoutingConfig::default());
        let fallback = Target {
            model: "fallback".to_string(),
            backend: "test".to_string(),
        };

        let result =
            router.resolve_for_agent("scout", "Find files", Some("explicit@backend"), &fallback);
        assert_eq!(result.model, "explicit");
        assert_eq!(result.backend, "backend");
    }

    #[test]
    fn test_model_annotation_parsing() {
        assert_eq!(
            RoutingContext::extract_model_annotation("Use @gpt-4o for this"),
            Some("gpt-4o".to_string())
        );
        assert_eq!(
            RoutingContext::extract_model_annotation("Route to @claude-3-5-sonnet@claude"),
            Some("claude-3-5-sonnet@claude".to_string())
        );
        assert_eq!(
            RoutingContext::extract_model_annotation("No annotation here"),
            None
        );
    }

    #[test]
    fn test_context_window_filtering() {
        let router = ModelRouter::new(ModelRoutingConfig::default());
        let fallback = Target {
            model: "gpt-4o".to_string(),
            backend: "chatgpt".to_string(),
        };

        let ctx = RoutingContext {
            estimated_tokens: Some(150000), // 150k tokens
            ..Default::default()
        };

        // Should pick a model with >= 150k context
        let result = router.resolve_with_context(RouteCategory::Coding, &ctx, &fallback);
        let caps = router.get_capabilities(&result.model);
        assert!(caps.context_window >= 150000);
    }

    #[test]
    fn test_unavailable_backend_filtering() {
        let router = ModelRouter::new(ModelRoutingConfig::default());
        let fallback = Target {
            model: "llama3:8b".to_string(),
            backend: "ollama".to_string(),
        };

        let ctx = RoutingContext {
            unavailable_backends: vec!["chatgpt".to_string(), "venice".to_string()],
            ..Default::default()
        };

        let result = router.resolve_with_context(RouteCategory::Default, &ctx, &fallback);
        assert!(!ctx.unavailable_backends.contains(&result.backend));
    }

    #[test]
    fn test_zdr_filtering() {
        let router = ModelRouter::new(ModelRoutingConfig::default());
        let fallback = Target {
            model: "claude-3-5-sonnet-latest".to_string(),
            backend: "claude".to_string(),
        };

        let ctx = RoutingContext {
            require_zdr: true,
            ..Default::default()
        };

        let result = router.resolve_with_context(RouteCategory::Coding, &ctx, &fallback);
        let caps = router.get_capabilities(&result.model);
        // Claude has ZDR
        assert!(caps.zdr || result.backend == "claude");
    }
}
