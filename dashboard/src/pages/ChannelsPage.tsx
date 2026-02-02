import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { MessageSquare, Radio } from "lucide-react";
import type { Channel } from "@/types/channel";

// Mock data - in production this would come from the API
const mockChannels: Channel[] = [
  {
    id: "ws-1",
    type: "websocket",
    name: "Dashboard",
    status: "connected",
    last_message_at: Date.now() - 5000,
    message_count: 42,
  },
  {
    id: "telegram-1",
    type: "telegram",
    name: "Telegram Bot",
    status: "disconnected",
    message_count: 0,
  },
  {
    id: "discord-1",
    type: "discord",
    name: "Discord Bot",
    status: "disconnected",
    message_count: 0,
  },
];

const channelIcons = {
  websocket: Radio,
  telegram: MessageSquare,
  discord: MessageSquare,
};

export default function ChannelsPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
        Channels
      </h1>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {mockChannels.map((channel) => {
          const Icon = channelIcons[channel.type];
          const statusVariant =
            channel.status === "connected"
              ? "success"
              : channel.status === "error"
                ? "error"
                : "neutral";

          return (
            <Card key={channel.id}>
              <div className="flex items-start gap-3">
                <div className="rounded-lg bg-gray-100 p-2 dark:bg-gray-800">
                  <Icon className="h-5 w-5 text-gray-600 dark:text-gray-400" />
                </div>
                <div className="flex-1">
                  <div className="flex items-center justify-between">
                    <h3 className="font-semibold text-gray-900 dark:text-gray-100">
                      {channel.name}
                    </h3>
                    <Badge variant={statusVariant} dot>
                      {channel.status}
                    </Badge>
                  </div>
                  <p className="text-sm capitalize text-gray-500 dark:text-gray-400">
                    {channel.type}
                  </p>
                </div>
              </div>

              <div className="mt-4 grid grid-cols-2 gap-4 text-sm">
                <div>
                  <span className="text-gray-500 dark:text-gray-400">
                    Messages
                  </span>
                  <p className="font-medium">{channel.message_count}</p>
                </div>
                <div>
                  <span className="text-gray-500 dark:text-gray-400">
                    Last Active
                  </span>
                  <p className="font-medium">
                    {channel.last_message_at
                      ? new Date(channel.last_message_at).toLocaleTimeString()
                      : "Never"}
                  </p>
                </div>
              </div>

              {channel.error && (
                <p className="mt-3 rounded bg-red-50 p-2 text-xs text-red-700 dark:bg-red-900/20 dark:text-red-300">
                  {channel.error}
                </p>
              )}
            </Card>
          );
        })}
      </div>
    </div>
  );
}
