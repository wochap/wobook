// Scriptable fake `chrome` for the popup harness. Tests set `window.__config`
// with page.addInitScript before this file runs:
//   tab: { id, url, title }, scrape: { title, description } | "throw",
//   responses: { [requestType]: Response }, nativeError: string
// Calls are recorded in window.__native, window.__created, window.__messages.
(() => {
  const config = window.__config ?? {};
  const event = () => {
    const listeners = [];
    return { listeners, addListener: (f) => listeners.push(f) };
  };
  const area = (store) => ({
    get: async (keys) => {
      const out = {};
      for (const k of [].concat(keys)) if (k in store) out[k] = store[k];
      return out;
    },
    set: async (items) => {
      Object.assign(store, items);
    },
  });
  window.__native = [];
  window.__created = [];
  window.__messages = [];
  window.__local = config.local ?? {};
  window.__closed = false;
  window.close = () => {
    window.__closed = true;
  };
  const tab = config.tab ?? { id: 1, url: "https://example.com/", title: "Example" };
  const onMessage = event();
  window.chrome = {
    runtime: {
      onMessage,
      sendNativeMessage: async (_name, request) => {
        window.__native.push(request);
        if (config.nativeError) throw new Error(config.nativeError);
        const response = config.responses?.[request.type] ?? {
          ok: false,
          error: { code: "not_found", message: "not found" },
        };
        // A successful add makes later `get` calls (the badge) find the bookmark.
        if (request.type === "add" && response.ok) {
          config.responses.get = { ok: true, result: response.result.bookmark };
        }
        return response;
      },
      sendMessage: (message) => {
        window.__messages.push(message);
        return new Promise((resolve) => {
          let answered = false;
          for (const l of onMessage.listeners) {
            if (l(message, {}, resolve) === true) answered = true;
          }
          if (!answered) resolve(undefined);
        });
      },
    },
    tabs: {
      query: async () => [tab],
      get: async () => tab,
      create: async (p) => {
        window.__created.push(p);
        return { id: 2, ...p };
      },
      onActivated: event(),
      onUpdated: event(),
      onRemoved: event(),
    },
    scripting: {
      executeScript: async () => {
        if (config.scrape === "throw") throw new Error("Cannot access contents of the page");
        return [{ result: config.scrape ?? { title: tab.title, description: "" } }];
      },
    },
    storage: { local: area(window.__local), session: area({}) },
    action: {
      setBadgeText: async (p) => {
        window.__badge = p;
      },
      setBadgeBackgroundColor: async () => {},
    },
  };
})();
