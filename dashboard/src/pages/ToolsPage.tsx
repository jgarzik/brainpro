import { useState } from "react";
import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { Tabs, TabPanel } from "@/components/ui/Tabs";
import {
  TOOLS,
  TOOL_CATEGORIES,
  TOOL_CATEGORY_MAP,
  type ToolName,
  type ToolCategory,
} from "@/constants/tools";

interface ToolInfo {
  name: ToolName;
  description: string;
  category: ToolCategory;
}

const toolDescriptions: Record<ToolName, string> = {
  [TOOLS.READ]: "Read file contents from the filesystem",
  [TOOLS.WRITE]: "Write content to a file",
  [TOOLS.EDIT]: "Edit a file by replacing text",
  [TOOLS.PATCH]: "Apply a patch to a file",
  [TOOLS.GLOB]: "Find files matching a glob pattern",
  [TOOLS.GREP]: "Search for patterns in files",
  [TOOLS.SEARCH]: "Search for code patterns",
  [TOOLS.BASH]: "Execute shell commands",
  [TOOLS.TASK]: "Spawn a subagent task",
  [TOOLS.ACTIVATE_SKILL]: "Activate a skill pack",
  [TOOLS.TODO]: "Manage todo items",
  [TOOLS.ASK_USER]: "Ask the user a question",
  [TOOLS.ENTER_PLAN_MODE]: "Enter planning mode",
  [TOOLS.EXIT_PLAN_MODE]: "Exit planning mode",
};

const tools: ToolInfo[] = Object.values(TOOLS).map((name) => ({
  name,
  description: toolDescriptions[name],
  category: TOOL_CATEGORY_MAP[name],
}));

const categoryTabs = [
  { key: "all", label: "All" },
  ...Object.values(TOOL_CATEGORIES).map((cat) => ({ key: cat, label: cat })),
];

export default function ToolsPage() {
  const [activeTab, setActiveTab] = useState("all");

  const filteredTools = tools.filter(
    (t) => activeTab === "all" || t.category === activeTab,
  );

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
        Tools
      </h1>

      <Tabs
        items={categoryTabs}
        activeKey={activeTab}
        onChange={setActiveTab}
      />

      <TabPanel active={true}>
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filteredTools.map((tool) => (
            <Card key={tool.name} padding="md">
              <div className="flex items-start justify-between">
                <h3 className="font-mono font-semibold text-gray-900 dark:text-gray-100">
                  {tool.name}
                </h3>
                <Badge variant="info">{tool.category}</Badge>
              </div>
              <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">
                {tool.description}
              </p>
            </Card>
          ))}
        </div>
      </TabPanel>
    </div>
  );
}
