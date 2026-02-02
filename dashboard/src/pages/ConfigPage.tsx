import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { getActionVariant } from "@/utils";
import type { Config } from "@/types/config";

// Mock config - in production this would come from the API
const mockConfig: Config = {
  persona: "MrCode",
  model: "claude-3-5-sonnet-latest",
  backend: "claude",
  backends: {
    claude: {
      name: "claude",
      api_url: "https://api.anthropic.com",
      default_model: "claude-3-5-sonnet-latest",
      enabled: true,
    },
    openai: {
      name: "openai",
      api_url: "https://api.openai.com",
      default_model: "gpt-4o",
      enabled: true,
    },
    venice: {
      name: "venice",
      api_url: "https://api.venice.ai",
      default_model: "llama-3.2-3b",
      enabled: true,
    },
  },
  policy: {
    mode: "ask",
    rules: [
      { tool: "Bash", action: "ask" },
      { tool: "Write", action: "ask" },
      { tool: "Read", action: "allow" },
      { tool: "Glob", action: "allow" },
    ],
  },
  context: {
    max_context_tokens: 128000,
    max_output_tokens: 8192,
    reserve_tokens: 4096,
  },
  agent: {
    max_turns: 12,
    max_iterations_per_turn: 10,
    doom_loop_threshold: 3,
  },
  cost: {
    enabled: true,
    warn_threshold_usd: 5.0,
    display_in_stats: true,
  },
  circuit_breaker: {
    enabled: true,
    failure_threshold: 5,
    recovery_timeout_secs: 30,
    half_open_probes: 3,
  },
};

function ConfigSection({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <Card title={title}>
      <div className="space-y-2 text-sm">{children}</div>
    </Card>
  );
}

function ConfigRow({
  label,
  value,
}: {
  label: string;
  value: React.ReactNode;
}) {
  return (
    <div className="flex justify-between py-1">
      <span className="text-gray-500 dark:text-gray-400">{label}</span>
      <span className="font-medium text-gray-900 dark:text-gray-100">
        {value}
      </span>
    </div>
  );
}

export default function ConfigPage() {
  const config = mockConfig;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
          Configuration
        </h1>
        <Badge variant="info">Read-only</Badge>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* General */}
        <ConfigSection title="General">
          <ConfigRow label="Persona" value={config.persona} />
          <ConfigRow
            label="Default Model"
            value={<code className="font-mono text-xs">{config.model}</code>}
          />
          <ConfigRow label="Default Backend" value={config.backend} />
        </ConfigSection>

        {/* Context Limits */}
        <ConfigSection title="Context Limits">
          <ConfigRow
            label="Max Context Tokens"
            value={config.context.max_context_tokens.toLocaleString()}
          />
          <ConfigRow
            label="Max Output Tokens"
            value={config.context.max_output_tokens.toLocaleString()}
          />
          <ConfigRow
            label="Reserve Tokens"
            value={config.context.reserve_tokens.toLocaleString()}
          />
        </ConfigSection>

        {/* Agent */}
        <ConfigSection title="Agent Loop">
          <ConfigRow label="Max Turns" value={config.agent.max_turns} />
          <ConfigRow
            label="Max Iterations/Turn"
            value={config.agent.max_iterations_per_turn}
          />
          <ConfigRow
            label="Doom Loop Threshold"
            value={config.agent.doom_loop_threshold}
          />
        </ConfigSection>

        {/* Circuit Breaker */}
        <ConfigSection title="Circuit Breaker">
          <ConfigRow
            label="Enabled"
            value={
              <Badge
                variant={config.circuit_breaker.enabled ? "success" : "neutral"}
              >
                {config.circuit_breaker.enabled ? "Yes" : "No"}
              </Badge>
            }
          />
          <ConfigRow
            label="Failure Threshold"
            value={config.circuit_breaker.failure_threshold}
          />
          <ConfigRow
            label="Recovery Timeout"
            value={`${config.circuit_breaker.recovery_timeout_secs}s`}
          />
          <ConfigRow
            label="Half-Open Probes"
            value={config.circuit_breaker.half_open_probes}
          />
        </ConfigSection>

        {/* Cost */}
        <ConfigSection title="Cost Tracking">
          <ConfigRow
            label="Enabled"
            value={
              <Badge variant={config.cost.enabled ? "success" : "neutral"}>
                {config.cost.enabled ? "Yes" : "No"}
              </Badge>
            }
          />
          <ConfigRow
            label="Warning Threshold"
            value={
              config.cost.warn_threshold_usd
                ? `$${config.cost.warn_threshold_usd}`
                : "None"
            }
          />
          <ConfigRow
            label="Display in Stats"
            value={config.cost.display_in_stats ? "Yes" : "No"}
          />
        </ConfigSection>

        {/* Policy Rules */}
        <ConfigSection title="Policy Rules">
          <div className="flex items-center gap-2 pb-2">
            <span className="text-gray-500 dark:text-gray-400">Mode:</span>
            <Badge variant="warning">{config.policy.mode}</Badge>
          </div>
          <div className="space-y-1 border-t border-gray-100 pt-2 dark:border-gray-800">
            {config.policy.rules.map((rule, idx) => (
              <div key={idx} className="flex items-center justify-between">
                <code className="font-mono text-xs">{rule.tool}</code>
                <Badge variant={getActionVariant(rule.action)}>
                  {rule.action}
                </Badge>
              </div>
            ))}
          </div>
        </ConfigSection>
      </div>

      {/* Backends */}
      <Card title="Backends">
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Object.values(config.backends).map((backend) => (
            <div
              key={backend.name}
              className="rounded-lg border border-gray-200 p-3 dark:border-gray-700"
            >
              <div className="flex items-center justify-between">
                <span className="font-medium">{backend.name}</span>
                <Badge variant={backend.enabled ? "success" : "neutral"} dot>
                  {backend.enabled ? "Enabled" : "Disabled"}
                </Badge>
              </div>
              <div className="mt-2 space-y-1 text-xs text-gray-500 dark:text-gray-400">
                <p className="truncate">URL: {backend.api_url}</p>
                {backend.default_model && (
                  <p>
                    Model:{" "}
                    <code className="font-mono">{backend.default_model}</code>
                  </p>
                )}
              </div>
            </div>
          ))}
        </div>
      </Card>
    </div>
  );
}
