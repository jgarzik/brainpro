import { useState } from "react";
import { Search, Grid, List } from "lucide-react";
import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { Input } from "@/components/ui/Input";
import { Button } from "@/components/ui/Button";
import type { SkillPack } from "@/types/skill";

// Mock data - in production this would come from the API
const mockSkills: SkillPack[] = [
  {
    id: "commit",
    name: "Git Commit",
    description: "Create well-formatted git commits",
    source: "builtin",
    active: true,
    frontmatter: {
      name: "commit",
      description: "Create well-formatted git commits",
    },
  },
  {
    id: "review-pr",
    name: "PR Review",
    description: "Review pull requests with detailed feedback",
    source: "builtin",
    active: true,
    frontmatter: { name: "review-pr", description: "Review pull requests" },
  },
  {
    id: "debugging",
    name: "Debugging",
    description: "Systematic debugging workflow",
    source: "builtin",
    active: true,
    frontmatter: {
      name: "debugging",
      description: "Debug issues systematically",
    },
  },
];

export default function SkillsPage() {
  const [search, setSearch] = useState("");
  const [viewMode, setViewMode] = useState<"grid" | "list">("grid");

  const filteredSkills = mockSkills.filter(
    (s) =>
      s.name.toLowerCase().includes(search.toLowerCase()) ||
      s.description.toLowerCase().includes(search.toLowerCase()),
  );

  const sourceVariant = (source: string) => {
    switch (source) {
      case "builtin":
        return "info";
      case "project":
        return "success";
      case "user":
        return "warning";
      default:
        return "neutral";
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
          Skills
        </h1>
        <div className="flex items-center gap-2">
          <Button
            variant={viewMode === "grid" ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setViewMode("grid")}
          >
            <Grid className="h-4 w-4" />
          </Button>
          <Button
            variant={viewMode === "list" ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setViewMode("list")}
          >
            <List className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* Search */}
      <div className="flex items-center gap-2">
        <Search className="h-4 w-4 text-gray-400" />
        <Input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search skills..."
          className="max-w-sm"
        />
      </div>

      {/* Skills */}
      {viewMode === "grid" ? (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filteredSkills.map((skill) => (
            <Card key={skill.id} padding="md">
              <div className="flex items-start justify-between">
                <h3 className="font-semibold text-gray-900 dark:text-gray-100">
                  {skill.name}
                </h3>
                <Badge
                  variant={
                    sourceVariant(skill.source) as
                      | "info"
                      | "success"
                      | "warning"
                      | "neutral"
                  }
                >
                  {skill.source}
                </Badge>
              </div>
              <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">
                {skill.description}
              </p>
              <div className="mt-3 flex items-center justify-between">
                <code className="text-xs text-gray-400">/{skill.id}</code>
                <Badge variant={skill.active ? "success" : "neutral"} dot>
                  {skill.active ? "Active" : "Inactive"}
                </Badge>
              </div>
            </Card>
          ))}
        </div>
      ) : (
        <div className="space-y-2">
          {filteredSkills.map((skill) => (
            <Card key={skill.id} padding="sm">
              <div className="flex items-center gap-4">
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <h3 className="font-semibold text-gray-900 dark:text-gray-100">
                      {skill.name}
                    </h3>
                    <Badge
                      variant={
                        sourceVariant(skill.source) as
                          | "info"
                          | "success"
                          | "warning"
                          | "neutral"
                      }
                    >
                      {skill.source}
                    </Badge>
                  </div>
                  <p className="text-sm text-gray-600 dark:text-gray-400">
                    {skill.description}
                  </p>
                </div>
                <code className="text-xs text-gray-400">/{skill.id}</code>
                <Badge variant={skill.active ? "success" : "neutral"} dot>
                  {skill.active ? "Active" : "Inactive"}
                </Badge>
              </div>
            </Card>
          ))}
        </div>
      )}

      {filteredSkills.length === 0 && (
        <Card>
          <p className="text-center text-gray-500 dark:text-gray-400">
            No skills found
          </p>
        </Card>
      )}
    </div>
  );
}
