import { api } from "./browser";
import { HOST_NAME, type Request, type Response } from "./protocol";

export type UiErrorKind =
  | "host_missing"
  | "host_crashed"
  | "daemon_down"
  | "hook_rejected"
  | "invalid_url"
  | "not_found"
  | "unknown";

export interface UiError {
  kind: UiErrorKind;
  message: string;
}

export type Outcome<T = unknown> = { ok: true; result: T } | { ok: false; error: UiError };

const README = "see extension/README.md#native-host";

export function fromThrown(e: unknown): UiError {
  const raw = e instanceof Error ? e.message : String(e);
  if (/not found|No such native application/i.test(raw)) {
    return { kind: "host_missing", message: `wobook native host is not installed (${README})` };
  }
  if (/exited|Error when communicating|disconnected port/i.test(raw)) {
    return {
      kind: "host_crashed",
      message: "wobook native host crashed; check the browser console",
    };
  }
  return { kind: "unknown", message: raw };
}

export function fromResponse(r: Response): Outcome {
  if (r.ok) return { ok: true, result: r.result };
  const code = r.error?.code;
  const message = r.error?.message ?? "unknown error";
  switch (code) {
    case "daemon_unavailable":
      return {
        ok: false,
        error: {
          kind: "daemon_down",
          message: "wobookd is not running. Start it: systemctl --user start wobookd",
        },
      };
    case "hook_rejected":
      return { ok: false, error: { kind: "hook_rejected", message } };
    case "invalid_url":
      return { ok: false, error: { kind: "invalid_url", message } };
    case "not_found":
      return { ok: false, error: { kind: "not_found", message } };
    default:
      return { ok: false, error: { kind: "unknown", message } };
  }
}

/** One native request; never throws. */
export async function callNative(request: Request): Promise<Outcome> {
  try {
    const r = (await api.runtime.sendNativeMessage(HOST_NAME, request)) as Response | undefined;
    if (!r)
      return { ok: false, error: fromThrown("Error when communicating with the native host") };
    return fromResponse(r);
  } catch (e) {
    return { ok: false, error: fromThrown(e) };
  }
}

/** Popup side: routes through the background worker. */
export async function viaWorker<T = unknown>(request: Request): Promise<Outcome<T>> {
  try {
    return (await api.runtime.sendMessage({ type: "native", request })) as Outcome<T>;
  } catch (e) {
    return { ok: false, error: fromThrown(e) };
  }
}
