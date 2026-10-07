// Chip tag editor: comma or Enter commits, spaces stay inside a tag,
// Backspace on an empty input removes the last chip.

export const normalizeTag = (t: string) => t.trim().toLowerCase();

export class TagEditor {
  tags: string[] = [];
  private suggestions: string[] = [];
  private active = 0;
  private readonly chips: HTMLElement;
  readonly input: HTMLInputElement;
  private readonly list: HTMLElement;

  constructor(
    root: HTMLElement,
    private source: () => string[],
  ) {
    this.chips = root.querySelector(".chips") as HTMLElement;
    this.input = root.querySelector("input") as HTMLInputElement;
    this.list = root.querySelector(".suggestions") as HTMLElement;
    root.addEventListener("click", (e) => {
      if (e.target === root || e.target === this.chips) this.input.focus();
    });
    this.input.addEventListener("input", () => {
      if (this.input.value.includes(",")) {
        const parts = this.input.value.split(",");
        this.input.value = parts.pop() ?? "";
        for (const p of parts) this.add(p);
      }
      this.suggest();
    });
    this.input.addEventListener("keydown", (e) => this.onKey(e));
    this.input.addEventListener("blur", () => setTimeout(() => this.hide(), 150));
  }

  set(tags: string[]) {
    this.tags = [];
    for (const t of tags) this.add(t);
  }

  add(raw: string) {
    const tag = normalizeTag(raw);
    if (tag && !this.tags.includes(tag)) this.tags.push(tag);
    this.renderChips();
  }

  /** Commits whatever is typed; returns the final tag set. */
  flush(): string[] {
    if (this.input.value.trim()) this.add(this.input.value);
    this.input.value = "";
    this.hide();
    return [...this.tags];
  }

  private onKey(e: KeyboardEvent) {
    if (e.ctrlKey || e.metaKey) return;
    const value = this.input.value;
    if (e.key === "Enter") {
      e.preventDefault();
      if (value.trim()) {
        this.add(value);
        this.input.value = "";
      }
      this.hide();
    } else if (e.key === "Tab" && this.suggestions.length && value.trim()) {
      e.preventDefault();
      this.accept(this.suggestions[this.active]);
    } else if (e.key === "Backspace" && value === "" && this.tags.length) {
      this.tags.pop();
      this.renderChips();
    } else if (e.key === "ArrowDown" && this.suggestions.length) {
      e.preventDefault();
      this.active = (this.active + 1) % this.suggestions.length;
      this.renderSuggestions();
    } else if (e.key === "ArrowUp" && this.suggestions.length) {
      e.preventDefault();
      this.active = (this.active - 1 + this.suggestions.length) % this.suggestions.length;
      this.renderSuggestions();
    } else if (e.key === "Escape" && this.suggestions.length) {
      e.preventDefault();
      this.hide();
    }
  }

  private accept(tag: string) {
    this.add(tag);
    this.input.value = "";
    this.hide();
    this.input.focus();
  }

  private suggest() {
    const q = normalizeTag(this.input.value);
    this.suggestions = q
      ? this.source()
          .filter((t) => t.startsWith(q) && !this.tags.includes(t))
          .slice(0, 6)
      : [];
    this.active = 0;
    this.renderSuggestions();
  }

  private hide() {
    this.suggestions = [];
    this.renderSuggestions();
  }

  private renderSuggestions() {
    this.list.hidden = this.suggestions.length === 0;
    this.list.replaceChildren(
      ...this.suggestions.map((t, i) => {
        const li = document.createElement("li");
        li.textContent = t;
        li.className = i === this.active ? "active" : "";
        li.addEventListener("mousedown", (e) => {
          e.preventDefault();
          this.accept(t);
        });
        return li;
      }),
    );
  }

  private renderChips() {
    this.chips.replaceChildren(
      ...this.tags.map((t) => {
        const chip = document.createElement("span");
        chip.className = "chip";
        chip.textContent = t;
        const x = document.createElement("button");
        x.type = "button";
        x.className = "chip-remove";
        x.setAttribute("aria-label", `Remove ${t}`);
        x.textContent = "×";
        x.addEventListener("click", () => {
          this.tags = this.tags.filter((o) => o !== t);
          this.renderChips();
        });
        chip.append(x);
        return chip;
      }),
    );
  }
}
