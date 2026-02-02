import { useState } from "react";
import { DollarSign, TrendingUp, Layers } from "lucide-react";
import { Card } from "@/components/ui/Card";
import { Tabs, TabPanel } from "@/components/ui/Tabs";
import {
  useCostStore,
  useCostByModel,
  useCostBySession,
} from "@/store/costStore";
import { formatCost, formatTokens } from "@/types/cost";
import { truncateId } from "@/utils";

function CostBarChart({
  data,
  maxValue,
}: {
  data: Array<{ label: string; value: number }>;
  maxValue: number;
}) {
  return (
    <div className="space-y-3">
      {data.map((item) => (
        <div key={item.label}>
          <div className="mb-1 flex justify-between text-sm">
            <span className="font-medium">{item.label}</span>
            <span className="text-gray-500 dark:text-gray-400">
              {formatCost(item.value)}
            </span>
          </div>
          <div className="h-3 overflow-hidden rounded-full bg-gray-100 dark:bg-gray-800">
            <div
              className="h-full rounded-full bg-blue-500 transition-all"
              style={{
                width: `${maxValue > 0 ? (item.value / maxValue) * 100 : 0}%`,
              }}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

const periodTabs = [
  { key: "today", label: "Today" },
  { key: "week", label: "This Week" },
  { key: "all", label: "All Time" },
];

export default function CostsPage() {
  const { totalCostUsd, totalInputTokens, totalOutputTokens } = useCostStore();
  const costByModel = useCostByModel();
  const costBySession = useCostBySession();

  const [period, setPeriod] = useState("all");

  // For now, we show all data regardless of period filter
  // In production, this would filter by date

  const modelData = costByModel.map((m) => ({
    label: m.model,
    value: m.costUsd,
  }));
  const maxModelCost = Math.max(...modelData.map((d) => d.value), 0.01);

  const sessionData = costBySession
    .sort((a, b) => b.costUsd - a.costUsd)
    .slice(0, 10)
    .map((s) => ({
      label: truncateId(s.sessionId),
      value: s.costUsd,
    }));
  const maxSessionCost = Math.max(...sessionData.map((d) => d.value), 0.01);

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
        Costs
      </h1>

      <Tabs items={periodTabs} activeKey={period} onChange={setPeriod} />

      <TabPanel active={true}>
        {/* Summary cards */}
        <div className="mb-6 grid gap-4 sm:grid-cols-3">
          <Card>
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-blue-100 p-2 dark:bg-blue-900/30">
                <DollarSign className="h-5 w-5 text-blue-600 dark:text-blue-400" />
              </div>
              <div>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  Total Cost
                </p>
                <p className="text-2xl font-semibold text-gray-900 dark:text-gray-100">
                  {formatCost(totalCostUsd)}
                </p>
              </div>
            </div>
          </Card>

          <Card>
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-emerald-100 p-2 dark:bg-emerald-900/30">
                <TrendingUp className="h-5 w-5 text-emerald-600 dark:text-emerald-400" />
              </div>
              <div>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  Input Tokens
                </p>
                <p className="text-2xl font-semibold text-gray-900 dark:text-gray-100">
                  {formatTokens(totalInputTokens)}
                </p>
              </div>
            </div>
          </Card>

          <Card>
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-amber-100 p-2 dark:bg-amber-900/30">
                <Layers className="h-5 w-5 text-amber-600 dark:text-amber-400" />
              </div>
              <div>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  Output Tokens
                </p>
                <p className="text-2xl font-semibold text-gray-900 dark:text-gray-100">
                  {formatTokens(totalOutputTokens)}
                </p>
              </div>
            </div>
          </Card>
        </div>

        <div className="grid gap-6 lg:grid-cols-2">
          {/* Cost by model */}
          <Card title="Cost by Model">
            {modelData.length > 0 ? (
              <CostBarChart data={modelData} maxValue={maxModelCost} />
            ) : (
              <p className="text-sm text-gray-500 dark:text-gray-400">
                No cost data available
              </p>
            )}
          </Card>

          {/* Cost by session */}
          <Card title="Top Sessions by Cost">
            {sessionData.length > 0 ? (
              <CostBarChart data={sessionData} maxValue={maxSessionCost} />
            ) : (
              <p className="text-sm text-gray-500 dark:text-gray-400">
                No session data available
              </p>
            )}
          </Card>
        </div>

        {/* Detailed table */}
        <Card title="Cost Breakdown by Model" className="mt-6">
          {costByModel.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-gray-200 dark:border-gray-700">
                    <th className="py-2 text-left font-medium text-gray-500 dark:text-gray-400">
                      Model
                    </th>
                    <th className="py-2 text-right font-medium text-gray-500 dark:text-gray-400">
                      Input Tokens
                    </th>
                    <th className="py-2 text-right font-medium text-gray-500 dark:text-gray-400">
                      Output Tokens
                    </th>
                    <th className="py-2 text-right font-medium text-gray-500 dark:text-gray-400">
                      Cost
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {costByModel.map((row) => (
                    <tr
                      key={row.model}
                      className="border-b border-gray-100 dark:border-gray-800"
                    >
                      <td className="py-2 font-mono">{row.model}</td>
                      <td className="py-2 text-right">
                        {formatTokens(row.inputTokens)}
                      </td>
                      <td className="py-2 text-right">
                        {formatTokens(row.outputTokens)}
                      </td>
                      <td className="py-2 text-right font-medium">
                        {formatCost(row.costUsd)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <p className="text-sm text-gray-500 dark:text-gray-400">
              No data available
            </p>
          )}
        </Card>
      </TabPanel>
    </div>
  );
}
