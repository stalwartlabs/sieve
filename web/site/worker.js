import init, { capabilities, compile, defaults, run, version } from "./pkg/sieve_web.js";

const ops = { capabilities, compile, defaults, run, version };

const ready = init().then(
  () => {
    self.postMessage({ type: "ready" });
    return true;
  },
  (err) => {
    self.postMessage({ type: "init-error", error: String(err && err.message ? err.message : err) });
    return false;
  },
);

self.onmessage = async (event) => {
  const { id, op, payload } = event.data;
  if (!(await ready)) return;
  try {
    const fn = ops[op];
    if (!fn) throw new Error(`Unknown operation ${op}`);
    const result = payload === undefined ? fn() : fn(payload);
    self.postMessage({ id, result });
  } catch (err) {
    self.postMessage({
      id,
      error: String(err && err.message ? err.message : err),
      fatal: err instanceof WebAssembly.RuntimeError,
    });
  }
};
