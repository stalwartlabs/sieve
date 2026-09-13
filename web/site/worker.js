import init, { capabilities, compile, defaults, run, version } from "./pkg/sieve_web.js";

const ops = { capabilities, compile, defaults, run, version };
const ready = init();

self.onmessage = async (event) => {
  const { id, op, payload } = event.data;
  try {
    await ready;
    const fn = ops[op];
    if (!fn) throw new Error(`Unknown operation ${op}`);
    const result = payload === undefined ? fn() : fn(payload);
    self.postMessage({ id, result });
  } catch (err) {
    self.postMessage({ id, error: String(err && err.message ? err.message : err), fatal: true });
  }
};
