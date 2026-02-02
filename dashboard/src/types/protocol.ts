/**
 * Protocol types matching src/protocol/client.rs
 */

/** Client roles */
export type ClientRole = "operator" | "node";

/** Client capabilities */
export interface ClientCapabilities {
  tools: string[];
  protocol_version: number;
}

/** Handshake: Client hello */
export interface Hello {
  role: ClientRole;
  device_id: string;
  caps: ClientCapabilities;
}

/** Handshake: Server challenge */
export interface Challenge {
  nonce: string;
}

/** Handshake: Client auth response */
export interface Auth {
  signature: string;
}

/** Policy info sent to client */
export interface PolicyInfo {
  mode: string;
  max_turns: number;
}

/** Handshake: Server welcome */
export interface Welcome {
  session_id: string;
  policy: PolicyInfo;
}

/** Error information in response */
export interface ErrorInfo {
  code: string;
  message: string;
}

/** Client → Gateway request frame */
export interface ClientRequest {
  type: "req";
  id: string;
  method: string;
  params: unknown;
}

/** Gateway → Client response frame */
export interface ClientResponse {
  type: "res";
  id: string;
  ok: boolean;
  payload?: unknown;
  error?: ErrorInfo;
}

/** Gateway → Client event frame */
export interface ClientEvent {
  type: "event";
  event: string;
  data: unknown;
  session_id?: string;
}

/** Handshake message types */
export interface HelloMessage {
  type: "hello";
  role: ClientRole;
  device_id: string;
  caps: ClientCapabilities;
}

export interface ChallengeMessage {
  type: "challenge";
  nonce: string;
}

export interface AuthMessage {
  type: "auth";
  signature: string;
}

export interface WelcomeMessage {
  type: "welcome";
  session_id: string;
  policy: PolicyInfo;
}

/** All possible incoming WebSocket message types */
export type WsMessage =
  | HelloMessage
  | ChallengeMessage
  | AuthMessage
  | WelcomeMessage
  | ClientRequest
  | ClientResponse
  | ClientEvent;

/** Parsed incoming message with discriminated union */
export type IncomingMessage =
  | { type: "challenge"; data: ChallengeMessage }
  | { type: "welcome"; data: WelcomeMessage }
  | { type: "res"; data: ClientResponse }
  | { type: "event"; data: ClientEvent };
