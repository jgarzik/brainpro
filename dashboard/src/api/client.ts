/**
 * Typed API client methods
 */

import { getWebSocket } from "./websocket";
import { METHODS } from "@/constants/methods";
import type { Session, Message } from "@/types/session";
import type { HealthStatus } from "@/types/health";

/** Chat message send parameters */
export interface ChatSendParams {
  message: string;
  session_id?: string | undefined;
}

/** Chat message send response */
export interface ChatSendResponse {
  session_id: string;
  message_id: string;
}

/** Session create parameters */
export interface SessionCreateParams {
  agent_id?: string;
}

/** Tool approve parameters */
export interface ToolApproveParams {
  session_id: string;
  turn_id: string;
  tool_call_id: string;
  approved: boolean;
}

/** Turn resume parameters */
export interface TurnResumeParams {
  session_id: string;
  turn_id: string;
  response: unknown;
}

/** API client namespace */
export const api = {
  /** Chat operations */
  chat: {
    /** Send a chat message */
    async send(params: ChatSendParams): Promise<ChatSendResponse> {
      return getWebSocket().send<ChatSendResponse>(METHODS.CHAT_SEND, params);
    },
  },

  /** Session operations */
  session: {
    /** Create a new session */
    async create(params?: SessionCreateParams): Promise<Session> {
      return getWebSocket().send<Session>(METHODS.SESSION_CREATE, params ?? {});
    },

    /** List all sessions */
    async list(): Promise<Session[]> {
      return getWebSocket().send<Session[]>(METHODS.SESSION_LIST);
    },

    /** Get session details */
    async get(sessionId: string): Promise<Session & { messages: Message[] }> {
      return getWebSocket().send<Session & { messages: Message[] }>(
        METHODS.SESSION_GET,
        {
          session_id: sessionId,
        },
      );
    },
  },

  /** Tool approval operations */
  tool: {
    /** Approve or deny a tool call */
    async approve(params: ToolApproveParams): Promise<void> {
      return getWebSocket().send<void>(METHODS.TOOL_APPROVE, params);
    },
  },

  /** Turn operations */
  turn: {
    /** Resume a paused turn with user response */
    async resume(params: TurnResumeParams): Promise<void> {
      return getWebSocket().send<void>(METHODS.TURN_RESUME, params);
    },
  },

  /** Health operations */
  health: {
    /** Get health status */
    async status(): Promise<HealthStatus> {
      return getWebSocket().send<HealthStatus>(METHODS.HEALTH_STATUS);
    },
  },

  /** Cron operations */
  cron: {
    /** List cron jobs */
    async list(): Promise<unknown[]> {
      return getWebSocket().send<unknown[]>(METHODS.CRON_LIST);
    },

    /** Add a cron job */
    async add(params: {
      schedule: string;
      command: string;
    }): Promise<{ id: string }> {
      return getWebSocket().send<{ id: string }>(METHODS.CRON_ADD, params);
    },

    /** Remove a cron job */
    async remove(id: string): Promise<void> {
      return getWebSocket().send<void>(METHODS.CRON_REMOVE, { id });
    },
  },
};
