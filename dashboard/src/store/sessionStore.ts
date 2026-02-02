/**
 * Session state store
 */

import { create } from "zustand";
import type {
  Session,
  Message,
  PendingApproval,
  PendingQuestion,
} from "@/types/session";

interface SessionStore {
  // Current active session
  currentSessionId: string | null;

  // All sessions by ID
  sessions: Map<string, Session>;

  // Messages by session ID
  messages: Map<string, Message[]>;

  // Streaming state
  streaming: boolean;
  streamBuffer: string;

  // Pending approval/question
  pendingApproval: PendingApproval | null;
  pendingQuestion: PendingQuestion | null;

  // Actions
  setCurrentSession: (sessionId: string | null) => void;
  addSession: (session: Session) => void;
  updateSession: (sessionId: string, updates: Partial<Session>) => void;
  removeSession: (sessionId: string) => void;

  addMessage: (sessionId: string, message: Message) => void;
  updateMessage: (
    sessionId: string,
    messageId: string,
    updates: Partial<Message>,
  ) => void;
  appendToMessage: (
    sessionId: string,
    messageId: string,
    content: string,
  ) => void;
  setMessages: (sessionId: string, messages: Message[]) => void;

  setStreaming: (streaming: boolean) => void;
  appendStreamBuffer: (content: string) => void;
  clearStreamBuffer: () => void;

  setPendingApproval: (approval: PendingApproval | null) => void;
  setPendingQuestion: (question: PendingQuestion | null) => void;

  clear: () => void;
}

export const useSessionStore = create<SessionStore>((set) => ({
  currentSessionId: null,
  sessions: new Map(),
  messages: new Map(),
  streaming: false,
  streamBuffer: "",
  pendingApproval: null,
  pendingQuestion: null,

  setCurrentSession: (sessionId) => set({ currentSessionId: sessionId }),

  addSession: (session) =>
    set((state) => {
      const sessions = new Map(state.sessions);
      sessions.set(session.id, session);
      return { sessions };
    }),

  updateSession: (sessionId, updates) =>
    set((state) => {
      const sessions = new Map(state.sessions);
      const existing = sessions.get(sessionId);
      if (existing) {
        sessions.set(sessionId, { ...existing, ...updates });
      }
      return { sessions };
    }),

  removeSession: (sessionId) =>
    set((state) => {
      const sessions = new Map(state.sessions);
      const messages = new Map(state.messages);
      sessions.delete(sessionId);
      messages.delete(sessionId);
      return {
        sessions,
        messages,
        currentSessionId:
          state.currentSessionId === sessionId ? null : state.currentSessionId,
      };
    }),

  addMessage: (sessionId, message) =>
    set((state) => {
      const messages = new Map(state.messages);
      const sessionMessages = [...(messages.get(sessionId) ?? []), message];
      messages.set(sessionId, sessionMessages);
      return { messages };
    }),

  updateMessage: (sessionId, messageId, updates) =>
    set((state) => {
      const messages = new Map(state.messages);
      const sessionMessages = messages.get(sessionId);
      if (sessionMessages) {
        const idx = sessionMessages.findIndex((m) => m.id === messageId);
        if (idx >= 0) {
          const newMessages = [...sessionMessages];
          newMessages[idx] = { ...sessionMessages[idx]!, ...updates };
          messages.set(sessionId, newMessages);
        }
      }
      return { messages };
    }),

  appendToMessage: (sessionId, messageId, content) =>
    set((state) => {
      const messages = new Map(state.messages);
      const sessionMessages = messages.get(sessionId);
      if (sessionMessages) {
        const idx = sessionMessages.findIndex((m) => m.id === messageId);
        if (idx >= 0) {
          const newMessages = [...sessionMessages];
          const existing = sessionMessages[idx]!;
          newMessages[idx] = {
            ...existing,
            content: existing.content + content,
          };
          messages.set(sessionId, newMessages);
        }
      }
      return { messages };
    }),

  setMessages: (sessionId, msgs) =>
    set((state) => {
      const messages = new Map(state.messages);
      messages.set(sessionId, msgs);
      return { messages };
    }),

  setStreaming: (streaming) => set({ streaming }),

  appendStreamBuffer: (content) =>
    set((state) => ({
      streamBuffer: state.streamBuffer + content,
    })),

  clearStreamBuffer: () => set({ streamBuffer: "" }),

  setPendingApproval: (approval) => set({ pendingApproval: approval }),

  setPendingQuestion: (question) => set({ pendingQuestion: question }),

  clear: () =>
    set({
      currentSessionId: null,
      sessions: new Map(),
      messages: new Map(),
      streaming: false,
      streamBuffer: "",
      pendingApproval: null,
      pendingQuestion: null,
    }),
}));

/** Selector: get current session */
export function useCurrentSession(): Session | null {
  return useSessionStore((state) => {
    const id = state.currentSessionId;
    return id ? (state.sessions.get(id) ?? null) : null;
  });
}

/** Selector: get messages for current session */
export function useCurrentMessages(): Message[] {
  return useSessionStore((state) => {
    const id = state.currentSessionId;
    return id ? (state.messages.get(id) ?? []) : [];
  });
}

/** Selector: get all sessions as array */
export function useAllSessions(): Session[] {
  return useSessionStore((state) => Array.from(state.sessions.values()));
}
