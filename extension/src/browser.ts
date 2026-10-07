// `browser` (Firefox) and `chrome` (Chromium MV3) both return promises for
// every API used here; this module narrows them to one typed surface.

export interface Tab {
  id?: number;
  url?: string;
  title?: string;
  status?: string;
}

interface Event<F> {
  addListener(listener: F): void;
}

interface StorageArea {
  get(keys: string | string[]): Promise<Record<string, unknown>>;
  set(items: Record<string, unknown>): Promise<void>;
}

export interface Api {
  runtime: {
    sendNativeMessage(application: string, message: unknown): Promise<unknown>;
    sendMessage(message: unknown): Promise<unknown>;
    onMessage: Event<
      (message: unknown, sender: unknown, sendResponse: (r: unknown) => void) => boolean | undefined
    >;
  };
  tabs: {
    query(q: { active: boolean; currentWindow: boolean }): Promise<Tab[]>;
    get(tabId: number): Promise<Tab>;
    create(p: { url: string }): Promise<Tab>;
    onActivated: Event<(info: { tabId: number }) => void>;
    onUpdated: Event<(tabId: number, change: { status?: string; url?: string }, tab: Tab) => void>;
    onRemoved: Event<(tabId: number) => void>;
  };
  scripting: {
    executeScript(p: {
      target: { tabId: number };
      func: () => unknown;
    }): Promise<{ result?: unknown }[]>;
  };
  storage: { local: StorageArea; session?: StorageArea };
  action: {
    setBadgeText(p: { text: string; tabId?: number }): Promise<void>;
    setBadgeBackgroundColor(p: { color: string; tabId?: number }): Promise<void>;
  };
}

const g = globalThis as unknown as { browser?: Api; chrome?: Api };
export const api: Api = (g.browser ?? g.chrome) as Api;
