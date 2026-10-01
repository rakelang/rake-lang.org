import type { CompileResult, WorkerReply, WorkerRequest } from "./protocol";

declare function importScripts(...urls: string[]): void;
declare function rakePlaygroundCompile(source: string, target: string): CompileResult;

self.addEventListener("message", (event: MessageEvent<WorkerRequest>) => {
  const request = event.data;
  if (request.type === "initialise") {
    importScripts(request.compiler);
    self.postMessage({ type: "ready" } satisfies WorkerReply);
    return;
  }
  try {
    const result = rakePlaygroundCompile(request.source, request.target);
    self.postMessage({ type: "result", id: request.id, result } satisfies WorkerReply);
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    self.postMessage({ type: "failure", id: request.id, message } satisfies WorkerReply);
  }
});
