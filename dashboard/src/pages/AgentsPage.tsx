import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import type { AgentInfo } from "@/types/persona";
import { MAIN_AGENT_TOOLS } from "@/constants/tools";

// Mock data - in production this would come from the API
const mockAgents: AgentInfo[] = [
  {
    id: "mrcode",
    name: "MrCode",
    type: "main",
    tools: MAIN_AGENT_TOOLS,
    description: "Primary coding assistant with full tool access",
  },
  {
    id: "mrbot",
    name: "MrBot",
    type: "main",
    tools: MAIN_AGENT_TOOLS,
    description: "Gateway persona with extended capabilities",
  },
  {
    id: "explorer",
    name: "Explorer",
    type: "subagent",
    tools: ["Read", "Glob", "Grep", "Search"],
    max_turns: 5,
    description: "Codebase exploration and search",
  },
  {
    id: "planner",
    name: "Planner",
    type: "subagent",
    tools: ["Read", "Glob", "Grep"],
    max_turns: 3,
    description: "Task planning and architecture",
  },
];

export default function AgentsPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
        Agents
      </h1>

      <div className="grid gap-4 sm:grid-cols-2">
        {mockAgents.map((agent) => (
          <Card key={agent.id} title={agent.name}>
            <div className="space-y-4">
              <div className="flex items-center gap-2">
                <Badge variant={agent.type === "main" ? "info" : "neutral"}>
                  {agent.type}
                </Badge>
                {agent.max_turns && (
                  <span className="text-xs text-gray-400">
                    Max {agent.max_turns} turns
                  </span>
                )}
              </div>

              {agent.description && (
                <p className="text-sm text-gray-600 dark:text-gray-400">
                  {agent.description}
                </p>
              )}

              <div>
                <h4 className="mb-2 text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">
                  Available Tools
                </h4>
                <div className="flex flex-wrap gap-1">
                  {agent.tools.map((tool) => (
                    <span
                      key={tool}
                      className="rounded bg-gray-100 px-2 py-0.5 font-mono text-xs text-gray-700 dark:bg-gray-800 dark:text-gray-300"
                    >
                      {tool}
                    </span>
                  ))}
                </div>
              </div>
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}
