import { type Tab, api } from "../browser";
import { type UiError, viaWorker } from "../native";
import type { Bookmark, Hit, TagCount } from "../protocol";
import { render, spans } from "./highlight";
import { TagEditor } from "./tag-editor";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const TAGS_TTL_MS = 60_000;
const RECENT_MAX = 20;

// ---- shared ----

function showError(el: HTMLElement, error: UiError | null) {
  el.hidden = !error;
  el.textContent = error?.message ?? "";
  el.dataset.kind = error?.kind ?? "";
}

const isWeb = (url?: string) => !!url && /^https?:\/\//i.test(url);

// ---- tabs ----

function switchTab(name: "save" | "search", explicit: boolean) {
  $("tab-save").setAttribute("aria-selected", String(name === "save"));
  $("tab-search").setAttribute("aria-selected", String(name === "search"));
  $("panel-save").hidden = name !== "save";
  $("panel-search").hidden = name !== "search";
  if (explicit) api.storage.local.set({ lastTab: name }).catch(() => {});
  if (name === "search") {
    $<HTMLInputElement>("query").focus();
    if (!searchStarted) {
      searchStarted = true;
      runSearch("");
    }
  } else {
    editor.input.focus();
  }
}

// ---- tag suggestions ----

let tagSource: string[] = [];

async function loadTags() {
  const session = api.storage.session;
  let counts: TagCount[] | undefined;
  try {
    const cached = (await session?.get("tagsCache"))?.tagsCache as
      | { at: number; tags: TagCount[] }
      | undefined;
    if (cached && Date.now() - cached.at < TAGS_TTL_MS) counts = cached.tags;
  } catch {}
  if (!counts) {
    const r = await viaWorker<TagCount[]>({ type: "tags" });
    counts = r.ok ? r.result : [];
    if (r.ok) session?.set({ tagsCache: { at: Date.now(), tags: counts } }).catch(() => {});
  }
  const recent = await recentTags();
  const byCount = [...counts].sort((a, b) => b.count - a.count).map((t) => t.tag);
  tagSource = [...new Set([...recent, ...byCount])];
}

async function recentTags(): Promise<string[]> {
  try {
    return ((await api.storage.local.get("recentTags")).recentTags as string[]) ?? [];
  } catch {
    return [];
  }
}

async function rememberTags(tags: string[]) {
  const recent = [...new Set([...tags, ...(await recentTags())])].slice(0, RECENT_MAX);
  await api.storage.local.set({ recentTags: recent }).catch(() => {});
}

// ---- save ----

const editor = new TagEditor($("tag-editor"), () => tagSource);
let tab: Tab | undefined;
let existing: Bookmark | undefined;
let saving = false;

function scrape() {
  const meta = (sel: string) => document.querySelector<HTMLMetaElement>(sel)?.content?.trim() ?? "";
  return {
    title: document.title,
    description: meta('meta[name="description"]') || meta('meta[property="og:description"]'),
  };
}

async function initSave() {
  const save = $<HTMLButtonElement>("save");
  [tab] = await api.tabs.query({ active: true, currentWindow: true });
  const url = tab?.url ?? "";
  $<HTMLInputElement>("url").value = url;
  $<HTMLInputElement>("title").value = tab?.title ?? "";
  if (!isWeb(url)) {
    save.disabled = true;
    $("save-status").textContent = "This page cannot be bookmarked";
    return;
  }
  const lookup = viaWorker<Bookmark>({ type: "get", url });
  loadTags();
  if (tab?.id !== undefined) {
    try {
      const [res] = await api.scripting.executeScript({ target: { tabId: tab.id }, func: scrape });
      const page = res?.result as { title: string; description: string } | undefined;
      if (page?.title) $<HTMLInputElement>("title").value = page.title;
      if (page?.description) $<HTMLTextAreaElement>("description").value = page.description;
    } catch {
      // Unscriptable page (store, PDF viewer): keep the tab title.
    }
  }
  const r = await lookup;
  if (r.ok && !r.result.deleted) {
    existing = r.result;
    const created = new Date(existing.created_ms).toLocaleDateString();
    const banner = $("saved-banner");
    banner.textContent = `Already saved on ${created}`;
    banner.hidden = false;
    if (existing.title) $<HTMLInputElement>("title").value = existing.title;
    if (existing.description) $<HTMLTextAreaElement>("description").value = existing.description;
    editor.set(existing.tags);
    save.textContent = "Update tags";
  } else if (!r.ok && r.error.kind !== "not_found") {
    showError($("save-error"), r.error);
  }
}

async function doSave() {
  const save = $<HTMLButtonElement>("save");
  if (saving || save.disabled) return;
  saving = true;
  save.disabled = true;
  showError($("save-error"), null);
  showError($("url-error"), null);
  const tags = editor.flush().sort();
  const url = $<HTMLInputElement>("url").value.trim();
  const r = existing
    ? await viaWorker({ type: "update", url: existing.url, tags, origin: "extension" })
    : await viaWorker({
        type: "add",
        url,
        title: $<HTMLInputElement>("title").value.trim() || undefined,
        description: $<HTMLTextAreaElement>("description").value.trim() || undefined,
        tags,
        fetch: false,
        merge: true,
        origin: "extension",
      });
  if (!r.ok) {
    showError(r.error.kind === "invalid_url" ? $("url-error") : $("save-error"), r.error);
    saving = false;
    save.disabled = false;
    return;
  }
  await rememberTags(tags);
  $("save-status").textContent = "✓ Saved";
  document.body.classList.add("saved");
  if (tab) api.runtime.sendMessage({ type: "badge:refresh", tab }).catch(() => {});
  setTimeout(() => window.close(), 600);
}

// ---- search ----

let searchStarted = false;
let results: Bookmark[] = [];
let selected = 0;
let debounce: ReturnType<typeof setTimeout> | undefined;
let generation = 0;

function displayUrl(url: string) {
  const m = /^[a-z][a-z0-9+.-]*:\/\//i.exec(url);
  return { text: url.slice(m ? m[0].length : 0), skip: m ? m[0].length : 0 };
}

async function runSearch(query: string) {
  const gen = ++generation;
  const q = query.trim();
  const r = q
    ? await viaWorker<Hit[]>({ type: "search", query: q, limit: 50 })
    : await viaWorker<Bookmark[]>({ type: "list", limit: 20 });
  if (gen !== generation) return;
  const list = $("results");
  if (!r.ok) {
    showError($("search-error"), r.error);
    list.replaceChildren();
    $("empty").hidden = true;
    results = [];
    return;
  }
  showError($("search-error"), null);
  const hits: Hit[] = q
    ? (r.result as Hit[])
    : (r.result as Bookmark[]).map((b) => ({
        bookmark: b,
        score: 0,
        indices: [],
        segments: { title: [0, 0], url: [0, 0], description: [0, 0], tags: [0, 0] },
      }));
  results = hits.map((h) => h.bookmark);
  selected = 0;
  $("empty").hidden = hits.length > 0;
  list.replaceChildren(...hits.map((h, i) => row(h, i)));
  markSelected();
}

function row(hit: Hit, i: number) {
  const b = hit.bookmark;
  const li = document.createElement("li");
  li.className = "result";
  li.dataset.index = String(i);
  const text = document.createElement("div");
  text.className = "result-text";
  const title = document.createElement("div");
  title.className = "result-title";
  render(title, spans(b.title || b.url, hit.segments.title, b.title ? hit.indices : []));
  const url = document.createElement("div");
  url.className = "result-url";
  const shown = displayUrl(b.url);
  render(url, spans(shown.text, hit.segments.url, hit.indices, shown.skip));
  text.append(title, url);
  if (b.tags.length) {
    const tags = document.createElement("div");
    tags.className = "result-tags";
    for (const t of b.tags) {
      const chip = document.createElement("span");
      chip.className = "chip tiny";
      chip.textContent = t;
      tags.append(chip);
    }
    text.append(tags);
  }
  const open = document.createElement("button");
  open.type = "button";
  open.className = "open";
  open.title = "Open in new tab";
  open.setAttribute("aria-label", "Open in new tab");
  open.textContent = "↗";
  open.addEventListener("click", (e) => {
    e.stopPropagation();
    openAt(i);
  });
  li.append(text, open);
  li.addEventListener("click", () => {
    selected = i;
    markSelected();
    copyAt(i);
  });
  return li;
}

function markSelected() {
  for (const el of $("results").children) {
    const on = Number((el as HTMLElement).dataset.index) === selected;
    el.classList.toggle("selected", on);
    if (on) el.scrollIntoView?.({ block: "nearest" });
  }
}

async function openAt(i: number) {
  const b = results[i];
  if (!b) return;
  await api.tabs.create({ url: b.url });
  window.close();
}

async function copyAt(i: number) {
  const b = results[i];
  if (!b) return;
  await navigator.clipboard.writeText(b.url);
  const el = $("results").children[i] as HTMLElement | undefined;
  el?.classList.add("copied");
  setTimeout(() => el?.classList.remove("copied"), 1000);
}

// ---- wiring ----

$("tab-save").addEventListener("click", () => switchTab("save", true));
$("tab-search").addEventListener("click", () => switchTab("search", true));
$("save").addEventListener("click", () => doSave());
$("save-form").addEventListener("submit", (e) => e.preventDefault());
$("panel-save").addEventListener("keydown", (e) => {
  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    doSave();
  }
});
$("query").addEventListener("input", () => {
  clearTimeout(debounce);
  const q = $<HTMLInputElement>("query").value;
  debounce = setTimeout(() => runSearch(q), 80);
});
$("query").addEventListener("keydown", (e) => {
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    if (!results.length) return;
    selected = (selected + (e.key === "ArrowDown" ? 1 : -1) + results.length) % results.length;
    markSelected();
  } else if (e.key === "Enter") {
    e.preventDefault();
    openAt(selected);
  } else if (e.key === "c" && (e.ctrlKey || e.metaKey)) {
    const input = e.target as HTMLInputElement;
    if (input.selectionStart === input.selectionEnd) {
      e.preventDefault();
      copyAt(selected);
    }
  }
});

// The shortcut cannot be told apart from a toolbar click, so the popup always
// opens on Save; `lastTab` only records explicit switches.
switchTab("save", false);
initSave();
