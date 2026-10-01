export type LaneTrace = {
  name: string;
  line: number;
  column: number;
  kind: string;
  values: string[];
  active: number | null;
};

export type CompileResult =
  | { ok: true; result: string | null; code: string; traces: LaneTrace[] }
  | { ok: false; message: string };

export type WorkerRequest =
  | { type: "initialise"; compiler: string }
  | { type: "compile"; id: number; source: string; target: string };

export type WorkerReply =
  | { type: "ready" }
  | { type: "result"; id: number; result: CompileResult }
  | { type: "failure"; id: number; message: string };
