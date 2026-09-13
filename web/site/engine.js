const RUN_TIMEOUT_MS = 8000;

export class Engine {
  constructor() {
    this.pending = new Map();
    this.nextId = 1;
    this.listeners = new Set();
    this.spawn();
  }

  onStatus(listener) {
    this.listeners.add(listener);
  }

  emit(state, detail) {
    for (const listener of this.listeners) listener(state, detail);
  }

  spawn() {
    this.worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
    this.worker.onmessage = (event) => this.settle(event.data);
    this.worker.onerror = (event) => {
      event.preventDefault();
      this.restart(`Engine failed to load: ${event.message || "unknown error"}`);
    };
  }

  restart(reason) {
    this.worker.terminate();
    for (const { reject, timer } of this.pending.values()) {
      clearTimeout(timer);
      reject(new Error(reason));
    }
    this.pending.clear();
    this.emit("restarted", reason);
    this.spawn();
  }

  settle({ id, result, error, fatal }) {
    const entry = this.pending.get(id);
    if (!entry) return;
    this.pending.delete(id);
    clearTimeout(entry.timer);
    if (error) {
      entry.reject(new Error(error));
      if (fatal) this.restart(error);
    } else {
      entry.resolve(result);
    }
  }

  call(op, payload, timeout = RUN_TIMEOUT_MS) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`The script did not finish within ${timeout / 1000} seconds and was stopped.`));
        this.restart("timeout");
      }, timeout);
      this.pending.set(id, { resolve, reject, timer });
      this.worker.postMessage({ id, op, payload });
    });
  }

  version() {
    return this.call("version");
  }

  capabilities() {
    return this.call("capabilities");
  }

  defaults() {
    return this.call("defaults");
  }

  compile(request) {
    return this.call("compile", request);
  }

  run(request) {
    return this.call("run", request);
  }
}
