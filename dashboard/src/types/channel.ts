/**
 * Channel types for messaging integrations
 */

/** Channel type */
export type ChannelType = "websocket" | "telegram" | "discord";

/** Channel status */
export type ChannelStatus = "connected" | "disconnected" | "error";

/** Channel information */
export interface Channel {
  id: string;
  type: ChannelType;
  name: string;
  status: ChannelStatus;
  last_message_at?: number;
  message_count: number;
  error?: string;
}

/** Channel configuration */
export interface ChannelConfig {
  type: ChannelType;
  enabled: boolean;
  config: Record<string, unknown>;
}
