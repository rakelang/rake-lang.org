import { Language, Parser, Query, type QueryCapture } from "web-tree-sitter";
import type { CompileResult, LaneTrace, WorkerReply, WorkerRequest } from "./protocol";

type Lesson = {
  heading: HTMLElement;
  source: string;
  instructions: Node[];
};

async function start(root: HTMLElement): Promise<void> {
  const editor = required<HTMLTextAreaElement>(root, "[data-editor]");
  const highlighting = required<HTMLElement>(root, "[data-highlighting]");
  const lessonText = required<HTMLElement>(root, "[data-lesson-text]");
  const lessonNumber = required<HTMLElement>(root, "[data-lesson-number]");
  const lessonTitle = required<HTMLElement>(root, "[data-lesson-title]");
  const target = required<HTMLSelectElement>(root, "[data-target]");
  const previous = required<HTMLButtonElement>(root, "[data-previous]");
  const next = required<HTMLButtonElement>(root, "[data-next]");
  const reset = required<HTMLButtonElement>(root, "[data-reset]");
  const run = required<HTMLButtonElement>(root, "[data-run]");
  const ligatures = required<HTMLInputElement>(root, "[data-ligatures]");
  const resultPanel = required<HTMLElement>(root, "[data-panel=result]");
  const lanesPanel = required<HTMLElement>(root, "[data-panel=lanes]");
  const codePanel = required<HTMLElement>(root, "[data-panel=code]");
  const messagesPanel = required<HTMLElement>(root, "[data-panel=messages]");
  const status = required<HTMLElement>(root, "[data-status]");
  const compiler = new CompilerWorker(root.dataset.worker!, root.dataset.compiler!);

  const lessons = collectLessons();
  if (lessons.length !== 12) {
    throw new Error(`The playground needs 12 lessons; found ${lessons.length}`);
  }

  let lessonIndex = lessonFromHash(lessons);
  let starter = "";
  let errorAt: { line: number; column: number } | null = null;
  let renderSerial = 0;
  let parser: Parser | null = null;
  let query: Query | null = null;
  let ready = false;
  run.disabled = true;
  target.disabled = true;
  status.textContent = "Loading the compiler and syntax grammar…";

  const showTab = (name: string): void => {
    for (const button of root.querySelectorAll<HTMLButtonElement>("[data-tab]")) {
      const selected = button.dataset.tab === name;
      button.setAttribute("aria-selected", String(selected));
      button.tabIndex = selected ? 0 : -1;
    }
    for (const panel of root.querySelectorAll<HTMLElement>("[data-panel]")) {
      panel.hidden = panel.dataset.panel !== name;
    }
  };

  const highlight = (): void => {
    if (!parser || !query) {
      highlighting.textContent = editor.value;
      highlighting.append(document.createTextNode("\n"));
      return;
    }
    highlighting.innerHTML = highlighted(parser, query, editor.value, errorAt);
    highlighting.append(document.createTextNode("\n"));
  };

  const compile = async (): Promise<void> => {
    if (!ready) {
      status.textContent = "The compiler is still loading…";
      return;
    }
    const attempt = ++renderSerial;
    errorAt = null;
    status.textContent = "Compiling…";
    const output = await compiler.compile(editor.value, target.value);
    if (attempt !== renderSerial) return;
    codePanel.textContent = output.ok ? output.code : "No code was emitted.";
    lanesPanel.replaceChildren();
    messagesPanel.replaceChildren();
    if (!output.ok) {
      const location = output.message.match(/playground\.rk:(\d+):(\d+):/);
      if (location) errorAt = { line: Number(location[1]), column: Number(location[2]) };
      messagesPanel.append(message("error", output.message));
      resultPanel.textContent = "The program did not run.";
      status.textContent = "Compiler message";
      root.classList.add("has-editor-error");
      showTab("messages");
      highlight();
      return;
    }

    root.classList.remove("has-editor-error");
    resultPanel.textContent = output.result === null
      ? "Compiled successfully. This definition has no main."
      : `main returned ${output.result}`;
    messagesPanel.append(message("success", "Compiled without errors."));
    renderLanes(lanesPanel, output.traces);
    status.textContent = "Compiled";
    showTab("result");
    highlight();
  };

  const loadLesson = (index: number): void => {
    lessonIndex = Math.max(0, Math.min(lessons.length - 1, index));
    const lesson = lessons[lessonIndex];
    starter = lesson.source;
    editor.value = starter;
    errorAt = null;
    root.classList.remove("has-editor-error");
    lessonNumber.textContent = `Lesson ${lessonIndex + 1} of ${lessons.length}`;
    lessonTitle.textContent = lesson.heading.textContent?.replace(/^\d+\.\s*/, "") ?? "";
    lessonText.replaceChildren(...lesson.instructions.map((node) => node.cloneNode(true)));
    previous.disabled = lessonIndex === 0;
    next.disabled = lessonIndex === lessons.length - 1;
    const wholeProgram = /(^|\n)(slow|run|record|state|embed|extern|const)\s/m.test(starter);
    if (wholeProgram) target.value = "wasm-simd128";
    required<HTMLElement>(root, "[data-target-note]").textContent = wholeProgram
      ? "WebAssembly covers general memory runs. SSE2, AVX2, AVX-512 and NEON support f32, i32 or u32 streams and single-column stack updates, including explicit byte/16-bit widening, i32s/u32s bitcasts, signed integer/float conversions and unsigned integer-to-float conversion. General native runs and cross-lane traversal operations remain work in progress. Results come from Rake's interpreter."
      : "Inspect vector code for SSE2, AVX2, AVX-512, NEON or WebAssembly. Results come from Rake's interpreter.";
    history.replaceState(null, "", `#lesson-${lessonIndex + 1}`);
    highlight();
    if (ready) void compile();
  };

  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-tab]")) {
    button.addEventListener("click", () => showTab(button.dataset.tab!));
    button.addEventListener("keydown", (event) => {
      if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
      event.preventDefault();
      const tabs = [...root.querySelectorAll<HTMLButtonElement>("[data-tab]")];
      const current = tabs.indexOf(button);
      const selected = event.key === "Home"
        ? 0
        : event.key === "End"
          ? tabs.length - 1
          : (current + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length;
      const tab = tabs[selected];
      showTab(tab.dataset.tab!);
      tab.focus();
    });
  }
  editor.addEventListener("input", () => {
    renderSerial += 1;
    errorAt = null;
    root.classList.remove("has-editor-error");
    status.textContent = "Edited; run to compile";
    highlight();
  });
  editor.addEventListener("scroll", () => {
    highlighting.parentElement!.scrollTop = editor.scrollTop;
    highlighting.parentElement!.scrollLeft = editor.scrollLeft;
  });
  editor.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
      event.preventDefault();
      void compile();
    }
    if (event.key === "Tab") {
      event.preventDefault();
      const start = editor.selectionStart;
      editor.setRangeText("  ", start, editor.selectionEnd, "end");
      editor.dispatchEvent(new Event("input"));
    }
  });
  run.addEventListener("click", () => void compile());
  reset.addEventListener("click", () => {
    editor.value = starter;
    highlight();
    void compile();
    editor.focus();
  });
  previous.addEventListener("click", () => loadLesson(lessonIndex - 1));
  next.addEventListener("click", () => loadLesson(lessonIndex + 1));
  target.addEventListener("change", () => void compile());
  ligatures.addEventListener("change", () => {
    root.classList.toggle("playground-no-ligatures", !ligatures.checked);
  });
  window.addEventListener("hashchange", () => {
    const selected = lessonFromHash(lessons);
    if (selected !== lessonIndex) loadLesson(selected);
  });

  loadLesson(lessonIndex);
  await Promise.all([
    Parser.init({ locateFile: () => root.dataset.treeSitter! }),
    compiler.ready(),
  ]);
  const language = await Language.load(root.dataset.rakeLanguage!);
  parser = new Parser();
  parser.setLanguage(language);
  const querySource = await fetch(root.dataset.rakeHighlights!).then((response) => {
    if (!response.ok) throw new Error(`Could not load the highlighting query (${response.status})`);
    return response.text();
  });
  query = new Query(language, querySource);
  ready = true;
  run.disabled = false;
  target.disabled = false;
  status.textContent = "Compiler ready";
  highlight();
  await compile();
}

class CompilerWorker {
  private worker: Worker;
  private serial = 0;
  private pending = new Map<number, (result: CompileResult) => void>();
  private readyPromise: Promise<void>;

  constructor(private readonly workerUrl: string, private readonly compilerUrl: string) {
    this.worker = this.create();
    this.readyPromise = this.initialise();
  }

  ready(): Promise<void> {
    return this.readyPromise;
  }

  async compile(source: string, target: string): Promise<CompileResult> {
    await this.readyPromise;
    const id = ++this.serial;
    return new Promise((resolve) => {
      this.pending.set(id, resolve);
      this.worker.postMessage({ type: "compile", id, source, target } satisfies WorkerRequest);
      window.setTimeout(() => {
        if (!this.pending.delete(id)) return;
        resolve({ ok: false, message: "The program ran for more than three seconds and was stopped." });
        this.worker.terminate();
        this.worker = this.create();
        this.readyPromise = this.initialise();
      }, 3_000);
    });
  }

  private create(): Worker {
    const worker = new Worker(this.workerUrl);
    worker.addEventListener("message", (event: MessageEvent<WorkerReply>) => {
      const reply = event.data;
      if (reply.type === "result") {
        this.pending.get(reply.id)?.(reply.result);
        this.pending.delete(reply.id);
      } else if (reply.type === "failure") {
        this.pending.get(reply.id)?.({ ok: false, message: reply.message });
        this.pending.delete(reply.id);
      }
    });
    return worker;
  }

  private initialise(): Promise<void> {
    return new Promise((resolve, reject) => {
      const ready = (event: MessageEvent<WorkerReply>): void => {
        if (event.data.type !== "ready") return;
        this.worker.removeEventListener("message", ready);
        resolve();
      };
      this.worker.addEventListener("message", ready);
      this.worker.addEventListener("error", (event) => reject(new Error(event.message)), { once: true });
      this.worker.postMessage({ type: "initialise", compiler: this.compilerUrl } satisfies WorkerRequest);
    });
  }
}

function collectLessons(): Lesson[] {
  const reference = document.querySelector<HTMLElement>("#playground-reference");
  if (!reference) return [];
  const lessons: Lesson[] = [];
  for (const heading of reference.querySelectorAll<HTMLElement>("h2[id]")) {
    if (!/^\d+-/.test(heading.id)) continue;
    const instructions: Node[] = [];
    let firstSource = "";
    let markedSource = "";
    let markedStarter = false;
    let sibling = heading.nextSibling;
    while (sibling) {
      if (sibling instanceof HTMLElement && sibling.tagName === "H2") break;
      if (sibling.nodeType === Node.COMMENT_NODE && sibling.textContent?.trim() === "playground-starter") {
        markedStarter = true;
      } else if (sibling instanceof HTMLElement) {
        if (sibling.matches("[data-playground-starter]")) {
          markedStarter = true;
          sibling = sibling.nextSibling;
          continue;
        }
        const code = sibling.matches("pre.code-rake") ? sibling.querySelector("code") : null;
        if (code) {
          if (firstSource === "") firstSource = code.textContent ?? "";
          if (markedStarter && markedSource === "") markedSource = code.textContent ?? "";
          markedStarter = false;
        } else if (["H3", "H4", "P", "UL", "OL", "BLOCKQUOTE"].includes(sibling.tagName)) {
          instructions.push(sibling);
        }
      }
      sibling = sibling.nextSibling;
    }
    const source = markedSource || firstSource;
    if (source) lessons.push({ heading, source, instructions });
  }
  return lessons;
}

function lessonFromHash(lessons: Lesson[]): number {
  const match = location.hash.match(/^#lesson-(\d+)$/);
  const index = match ? Number(match[1]) - 1 : 0;
  return index >= 0 && index < lessons.length ? index : 0;
}

function highlighted(
  parser: Parser,
  query: Query,
  source: string,
  errorAt: { line: number; column: number } | null,
): string {
  const tree = parser.parse(source);
  if (!tree) return escapeHtml(source);
  const captures = query.captures(tree.rootNode);
  const ranges = nonOverlapping(captures);
  const error = errorAt ? errorRange(source, errorAt.line, errorAt.column) : null;
  const boundaries = new Set<number>([0, source.length]);
  for (const range of ranges) {
    boundaries.add(range.start);
    boundaries.add(range.end);
  }
  if (error) {
    boundaries.add(error.start);
    boundaries.add(error.end);
  }
  const points = [...boundaries].sort((a, b) => a - b);
  let html = "";
  let rangeIndex = 0;
  for (let i = 0; i + 1 < points.length; i += 1) {
    const start = points[i];
    const end = points[i + 1];
    while (rangeIndex < ranges.length && ranges[rangeIndex].end <= start) rangeIndex += 1;
    const range = ranges[rangeIndex];
    const classes: string[] = [];
    if (range && start >= range.start && end <= range.end) {
      classes.push(`tok-${range.name.replaceAll(".", "-")}`);
    }
    if (error && start < error.end && end > error.start) classes.push("editor-error-token");
    const text = escapeHtml(source.slice(start, end));
    html += classes.length ? `<span class="${classes.join(" ")}">${text}</span>` : text;
  }
  tree.delete();
  return html;
}

function nonOverlapping(captures: QueryCapture[]): { start: number; end: number; name: string }[] {
  // web-tree-sitter indices use JavaScript UTF-16 units, including in comments.
  const ordered = captures.map((capture) => ({
    start: capture.node.startIndex,
    end: capture.node.endIndex,
    name: capture.name,
  })).sort((a, b) => a.start - b.start || a.end - b.end);
  let end = 0;
  return ordered.filter((range) => {
    if (range.start < end) return false;
    end = range.end;
    return true;
  });
}

function errorRange(source: string, line: number, column: number): { start: number; end: number } {
  const lines = source.split("\n");
  const start = lines.slice(0, Math.max(0, line - 1)).reduce((sum, value) => sum + value.length + 1, 0) + column;
  const rest = source.slice(start);
  const token = rest.match(/^[A-Za-z_][A-Za-z_0-9]*|^\S/)?.[0] ?? "";
  return { start, end: start + Math.max(1, token.length) };
}

function renderLanes(panel: HTMLElement, traces: LaneTrace[]): void {
  if (traces.length === 0) {
    panel.append(message("plain", "This program did not produce a lane trace. Vector-only definitions can still be inspected in Code."));
    return;
  }
  const list = document.createElement("ol");
  list.className = "lane-traces";
  for (const trace of traces) {
    const item = document.createElement("li");
    const label = document.createElement("p");
    label.className = "lane-trace-label";
    label.textContent = `${trace.name.replace(/\$\d+$/, "")} · ${trace.kind} · line ${trace.line}`;
    const lanes = document.createElement("div");
    lanes.className = "lane-row";
    lanes.setAttribute("aria-label", `${trace.name}: ${trace.values.join(", ")}`);
    trace.values.forEach((value, index) => {
      const cell = document.createElement("span");
      cell.className = "lane-cell";
      if (trace.kind === "mask") cell.classList.add(value === "yes" ? "is-true" : "is-false");
      if (trace.active !== null && index >= trace.active) cell.classList.add("is-inactive");
      cell.textContent = value;
      lanes.append(cell);
    });
    item.append(label, lanes);
    list.append(item);
  }
  panel.append(list);
}

function message(kind: string, text: string): HTMLElement {
  const paragraph = document.createElement("p");
  paragraph.className = `playground-message playground-message-${kind}`;
  paragraph.textContent = text;
  return paragraph;
}

function required<T extends Element>(root: ParentNode, selector: string): T {
  const element = root.querySelector<T>(selector);
  if (!element) throw new Error(`Missing playground element ${selector}`);
  return element;
}

function escapeHtml(text: string): string {
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

const root = document.querySelector<HTMLElement>("[data-rake-playground]");
if (root) {
  void start(root).catch((error: unknown) => {
    const detail = error instanceof Error ? error.message : String(error);
    const status = root.querySelector<HTMLElement>("[data-status]");
    const messages = root.querySelector<HTMLElement>("[data-panel=messages]");
    if (status) status.textContent = "The playground could not start";
    if (messages) messages.replaceChildren(message("error", detail));
    for (const panel of root.querySelectorAll<HTMLElement>("[data-panel]")) {
      panel.hidden = panel !== messages;
    }
    const messagesTab = root.querySelector<HTMLButtonElement>("[data-tab=messages]");
    if (messagesTab) {
      for (const tab of root.querySelectorAll<HTMLButtonElement>("[data-tab]")) {
        const selected = tab === messagesTab;
        tab.setAttribute("aria-selected", String(selected));
        tab.tabIndex = selected ? 0 : -1;
      }
    }
  });
}
