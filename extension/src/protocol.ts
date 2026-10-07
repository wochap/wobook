// TypeScript mirror of wobook_core::protocol (requests used by the extension).

export interface Bookmark {
  url: string;
  title: string;
  description: string;
  tags: string[];
  created_ms: number;
  updated_ms: number;
  deleted: boolean;
}

export interface Segments {
  title: [number, number];
  url: [number, number];
  description: [number, number];
  tags: [number, number];
}

export interface Hit {
  bookmark: Bookmark;
  score: number;
  indices: number[];
  segments: Segments;
}

export interface TagCount {
  tag: string;
  count: number;
}

export type Request =
  | { type: "ping" }
  | { type: "get"; url: string }
  | {
      type: "add";
      url: string;
      title?: string;
      description?: string;
      tags: string[];
      fetch: boolean;
      merge: boolean;
      origin: string;
    }
  | { type: "update"; url: string; tags: string[]; origin?: string }
  | { type: "search"; query: string; limit: number; tags?: string[] }
  | { type: "list"; limit: number; tags?: string[] }
  | { type: "tags" };

export type ErrorCode =
  | "invalid_request"
  | "invalid_url"
  | "not_found"
  | "exists"
  | "hook_rejected"
  | "io"
  | "internal"
  // Client-side, emitted by `wobook native-host` only.
  | "daemon_unavailable"
  | "response_too_large";

export interface Response {
  ok: boolean;
  result?: unknown;
  error?: { code: ErrorCode; message: string };
}

export const HOST_NAME = "dev.wochap.wobook";
