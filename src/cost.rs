//! Cost tracking module for token economics.
//!
//! Tracks token usage and costs across operations, turns, and sessions.
//! Supports per-model pricing configuration with sensible defaults.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pricing for a single model (per 1M tokens in USD)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelPricing {
    /// Cost per 1M input tokens
    pub input: f64,
    /// Cost per 1M output tokens
    pub output: f64,
}

impl ModelPricing {
    pub fn new(input: f64, output: f64) -> Self {
        Self { input, output }
    }

    /// Calculate cost for given token counts
    pub fn calculate(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.input;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.output;
        input_cost + output_cost
    }
}

/// Configuration for cost tracking
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CostConfig {
    /// Enable cost tracking and display
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Warn when session cost exceeds this threshold (USD)
    #[serde(default)]
    pub warn_threshold_usd: Option<f64>,
    /// Show cost in the stats line after each turn
    #[serde(default = "default_true")]
    pub display_in_stats: bool,
}

fn default_true() -> bool {
    true
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            warn_threshold_usd: None,
            display_in_stats: true,
        }
    }
}

/// Cost for a single LLM operation
#[derive(Debug, Clone, Serialize)]
pub struct OperationCost {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
}

impl OperationCost {
    pub fn new(model: String, input_tokens: u64, output_tokens: u64, cost_usd: f64) -> Self {
        Self {
            model,
            input_tokens,
            output_tokens,
            cost_usd,
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Aggregated costs for a single turn (user message -> assistant response)
#[derive(Debug, Clone, Default)]
pub struct TurnCost {
    pub turn_number: u32,
    pub operations: Vec<OperationCost>,
}

impl TurnCost {
    pub fn new(turn_number: u32) -> Self {
        Self {
            turn_number,
            operations: Vec::new(),
        }
    }

    pub fn add_operation(&mut self, op: OperationCost) {
        self.operations.push(op);
    }

    pub fn total_tokens(&self) -> u64 {
        self.operations.iter().map(|op| op.total_tokens()).sum()
    }

    pub fn total_cost(&self) -> f64 {
        self.operations.iter().map(|op| op.cost_usd).sum()
    }

    #[allow(dead_code)] // For future detailed reporting
    pub fn input_tokens(&self) -> u64 {
        self.operations.iter().map(|op| op.input_tokens).sum()
    }

    #[allow(dead_code)] // For future detailed reporting
    pub fn output_tokens(&self) -> u64 {
        self.operations.iter().map(|op| op.output_tokens).sum()
    }
}

/// Session-level cost tracker
#[derive(Debug, Clone)]
pub struct SessionCosts {
    #[allow(dead_code)] // For future session persistence
    session_id: String,
    turns: Vec<TurnCost>,
    pricing: PricingTable,
}

impl SessionCosts {
    pub fn new(session_id: String, pricing: PricingTable) -> Self {
        Self {
            session_id,
            turns: Vec::new(),
            pricing,
        }
    }

    /// Record an LLM operation and return the cost
    pub fn record_operation(
        &mut self,
        turn_number: u32,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> OperationCost {
        let cost_usd = self.pricing.calculate(model, input_tokens, output_tokens);
        let op = OperationCost::new(model.to_string(), input_tokens, output_tokens, cost_usd);

        // Find or create the turn
        if let Some(turn) = self.turns.iter_mut().find(|t| t.turn_number == turn_number) {
            turn.add_operation(op.clone());
        } else {
            let mut turn = TurnCost::new(turn_number);
            turn.add_operation(op.clone());
            self.turns.push(turn);
        }

        op
    }

    /// Merge costs from a subagent into the current turn
    #[allow(dead_code)] // For future parallel subagent support
    pub fn merge_operations(&mut self, turn_number: u32, ops: Vec<OperationCost>) {
        if let Some(turn) = self.turns.iter_mut().find(|t| t.turn_number == turn_number) {
            for op in ops {
                turn.add_operation(op);
            }
        } else {
            let mut turn = TurnCost::new(turn_number);
            for op in ops {
                turn.add_operation(op);
            }
            self.turns.push(turn);
        }
    }

    #[allow(dead_code)] // For future session persistence
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn turns(&self) -> &[TurnCost] {
        &self.turns
    }

    pub fn total_tokens(&self) -> u64 {
        self.turns.iter().map(|t| t.total_tokens()).sum()
    }

    pub fn total_cost(&self) -> f64 {
        self.turns.iter().map(|t| t.total_cost()).sum()
    }

    #[allow(dead_code)] // For future detailed reporting
    pub fn input_tokens(&self) -> u64 {
        self.turns.iter().map(|t| t.input_tokens()).sum()
    }

    #[allow(dead_code)] // For future detailed reporting
    pub fn output_tokens(&self) -> u64 {
        self.turns.iter().map(|t| t.output_tokens()).sum()
    }

    /// Get cost breakdown by model
    pub fn cost_by_model(&self) -> HashMap<String, (u64, f64)> {
        let mut result: HashMap<String, (u64, f64)> = HashMap::new();
        for turn in &self.turns {
            for op in &turn.operations {
                let entry = result.entry(op.model.clone()).or_insert((0, 0.0));
                entry.0 += op.total_tokens();
                entry.1 += op.cost_usd;
            }
        }
        result
    }
}

/// Pricing table with model-specific costs
#[derive(Debug, Clone)]
pub struct PricingTable {
    models: HashMap<String, ModelPricing>,
    default_pricing: ModelPricing,
}

impl Default for PricingTable {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl PricingTable {
    /// Create a pricing table with default model prices
    pub fn with_defaults() -> Self {
        let mut models = HashMap::new();

        // OpenAI models
        models.insert("gpt-4o".to_string(), ModelPricing::new(2.50, 10.00));
        models.insert("gpt-4o-mini".to_string(), ModelPricing::new(0.15, 0.60));
        models.insert("gpt-4-turbo".to_string(), ModelPricing::new(10.00, 30.00));
        models.insert("gpt-3.5-turbo".to_string(), ModelPricing::new(0.50, 1.50));
        models.insert("o1".to_string(), ModelPricing::new(15.00, 60.00));
        models.insert("o1-mini".to_string(), ModelPricing::new(3.00, 12.00));
        models.insert("o1-preview".to_string(), ModelPricing::new(15.00, 60.00));

        // Anthropic models
        models.insert(
            "claude-3-5-sonnet-latest".to_string(),
            ModelPricing::new(3.00, 15.00),
        );
        models.insert(
            "claude-3-5-sonnet-20241022".to_string(),
            ModelPricing::new(3.00, 15.00),
        );
        models.insert(
            "claude-3-5-haiku-latest".to_string(),
            ModelPricing::new(0.80, 4.00),
        );
        models.insert(
            "claude-3-opus-latest".to_string(),
            ModelPricing::new(15.00, 75.00),
        );

        // Venice / free tier models (approximate as free)
        models.insert(
            "qwen3-235b-a22b-instruct-2507".to_string(),
            ModelPricing::new(0.00, 0.00),
        );
        models.insert(
            "llama-3.3-70b".to_string(),
            ModelPricing::new(0.00, 0.00),
        );

        // Ollama / local models (no cost)
        models.insert("llama3".to_string(), ModelPricing::new(0.00, 0.00));
        models.insert("llama3:8b".to_string(), ModelPricing::new(0.00, 0.00));
        models.insert("codellama".to_string(), ModelPricing::new(0.00, 0.00));

        Self {
            models,
            default_pricing: ModelPricing::new(1.00, 3.00), // Conservative default
        }
    }

    /// Create from user config, merging with defaults
    pub fn from_config(user_pricing: &HashMap<String, ModelPricing>) -> Self {
        let mut table = Self::with_defaults();
        for (model, pricing) in user_pricing {
            table.models.insert(model.clone(), pricing.clone());
        }
        table
    }

    /// Get pricing for a model (falls back to default if unknown)
    pub fn get(&self, model: &str) -> &ModelPricing {
        // Try exact match first
        if let Some(pricing) = self.models.get(model) {
            return pricing;
        }

        // Try prefix matching for versioned models (e.g., "gpt-4o-2024-08-06" -> "gpt-4o")
        for (name, pricing) in &self.models {
            if model.starts_with(name) {
                return pricing;
            }
        }

        &self.default_pricing
    }

    /// Calculate cost for given model and token counts
    pub fn calculate(&self, model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
        self.get(model).calculate(input_tokens, output_tokens)
    }

    /// Add or update pricing for a model
    #[allow(dead_code)] // For future runtime pricing updates
    pub fn set(&mut self, model: &str, pricing: ModelPricing) {
        self.models.insert(model.to_string(), pricing);
    }
}

/// Format a cost value for display
pub fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("${:.4}", cost)
    } else if cost < 1.0 {
        format!("${:.3}", cost)
    } else {
        format!("${:.2}", cost)
    }
}

/// Format token count for display
pub fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_pricing_calculation() {
        let pricing = ModelPricing::new(2.50, 10.00);
        // 1000 input tokens, 500 output tokens
        let cost = pricing.calculate(1000, 500);
        // (1000/1M) * 2.50 + (500/1M) * 10.00 = 0.0025 + 0.005 = 0.0075
        assert!((cost - 0.0075).abs() < 0.0001);
    }

    #[test]
    fn test_pricing_table_prefix_matching() {
        let table = PricingTable::with_defaults();
        // Versioned model should match base model
        let pricing = table.get("gpt-4o-2024-08-06");
        assert_eq!(pricing.input, 2.50);
    }

    #[test]
    fn test_session_costs() {
        let pricing = PricingTable::with_defaults();
        let mut session = SessionCosts::new("test-session".to_string(), pricing);

        session.record_operation(1, "gpt-4o-mini", 1000, 500);
        session.record_operation(1, "gpt-4o-mini", 500, 200);

        assert_eq!(session.total_tokens(), 2200);
        assert!(session.total_cost() > 0.0);

        let by_model = session.cost_by_model();
        assert!(by_model.contains_key("gpt-4o-mini"));
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.001), "$0.0010");
        assert_eq!(format_cost(0.05), "$0.050");
        assert_eq!(format_cost(1.23), "$1.23");
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5k");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }
}
