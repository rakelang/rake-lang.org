"use strict";
(() => {
  // src/worker.ts
  self.addEventListener("message", (event) => {
    const request = event.data;
    if (request.type === "initialise") {
      importScripts(request.compiler);
      self.postMessage({ type: "ready" });
      return;
    }
    try {
      const result = rakePlaygroundCompile(request.source, request.target);
      self.postMessage({ type: "result", id: request.id, result });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      self.postMessage({ type: "failure", id: request.id, message });
    }
  });
})();
