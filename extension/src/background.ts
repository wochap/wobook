import { type Tab, api } from "./browser";
import { callNative } from "./native";
import type { Bookmark, Request } from "./protocol";

const ACCENT = "#89b4fa";
const cache = new Map<number, { url: string; bookmarked: boolean }>();

const isWeb = (url?: string): url is string => !!url && /^https?:\/\//.test(url);

async function setBadge(tabId: number, bookmarked: boolean) {
  try {
    await api.action.setBadgeText({ text: bookmarked ? "✓" : "", tabId });
    if (bookmarked) await api.action.setBadgeBackgroundColor({ color: ACCENT, tabId });
  } catch (e) {
    console.debug("wobook badge", e);
  }
}

async function refresh(tab: Tab, force = false) {
  if (tab.id === undefined) return;
  const tabId = tab.id;
  if (!isWeb(tab.url)) {
    cache.delete(tabId);
    await setBadge(tabId, false);
    return;
  }
  const cached = cache.get(tabId);
  if (!force && cached && cached.url === tab.url) {
    await setBadge(tabId, cached.bookmarked);
    return;
  }
  const r = await callNative({ type: "get", url: tab.url });
  // Any error: silent, cleared badge.
  const bookmarked = r.ok && !(r.result as Bookmark).deleted;
  cache.set(tabId, { url: tab.url, bookmarked });
  await setBadge(tabId, bookmarked);
}

api.tabs.onActivated.addListener(({ tabId }) => {
  api.tabs.get(tabId).then(
    (tab) => refresh(tab),
    () => {},
  );
});

api.tabs.onUpdated.addListener((_tabId, change, tab) => {
  if (change.status === "complete") refresh(tab);
});

api.tabs.onRemoved.addListener((tabId) => {
  cache.delete(tabId);
});

type Message = { type: "native"; request: Request } | { type: "badge:refresh"; tab: Tab };

api.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  const m = message as Message;
  if (m?.type === "native") {
    callNative(m.request).then(sendResponse);
    return true;
  }
  if (m?.type === "badge:refresh") {
    refresh(m.tab, true).then(() => sendResponse(true));
    return true;
  }
  return undefined;
});
