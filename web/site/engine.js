const RUN_TIMEOUT_MS = 8000;
const MAX_RESTARTS = 3;
const RESTART_WINDOW_MS = 60000;
const BASE_BACKOFF_MS = 500;

export class EngineUnavailable extends Error {}

export class Engine {
  constructor() {
    this.pending = new Map();
    this.nextId = 1;
    this.listeners = new Set();
    this.restarts = [];
    this.failed = null;
    this.spawn();
  }

  onStatus(listener) {
    this.listeners.add(listener);
  }

  emit(state, detail) {
    for (const listener of this.listeners) listener(state, detail);
  }

  spawn() {
    const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
    this.worker = worker;
    this.ready = new Promise((resolve, reject) => {
      this.resolveReady = resolve;
      this.rejectReady = reject;
    });
    this.ready.catch(() => {});
    worker.onmessage = (event) => {
      if (worker !== this.worker) return;
      const data = event.data;
      if (data.type === "ready") {
        this.resolveReady();
      } else if (data.type === "init-error") {
        this.crash(`Engine failed to load: ${data.error}`, { loadFailure: true });
      } else {
        this.settle(data);
      }
    };
    worker.onerror = (event) => {
      event.preventDefault();
      if (worker === this.worker) {
        this.crash(`Engine failed to load: ${event.message || "the worker script could not be loaded"}`, { loadFailure: true });
      }
    };
  }

  crash(reason, { loadFailure = false } = {}) {
    this.worker.terminate();
    this.rejectReady(new EngineUnavailable(reason));
    for (const { reject, timer } of this.pending.values()) {
      clearTimeout(timer);
      reject(new EngineUnavailable(reason));
    }
    this.pending.clear();

    let delay = 0;
    if (loadFailure) {
      const now = Date.now();
      this.restarts = this.restarts.filter((time) => now - time < RESTART_WINDOW_MS);
      if (this.restarts.length >= MAX_RESTARTS) {
        this.failed = reason;
        this.emit("failed", reason);
        return;
      }
      this.restarts.push(now);
      delay = BASE_BACKOFF_MS * 2 ** (this.restarts.length - 1);
    }
    this.emit("restarting", reason);
    const failedReady = this.ready;
    this.ready = new Promise((resolve, reject) => {
      setTimeout(() => {
        this.spawn();
        this.ready.then(resolve, reject);
      }, delay);
    });
    this.ready.catch(() => {});
    failedReady.catch(() => {});
  }

  settle({ id, result, error, fatal }) {
    const entry = this.pending.get(id);
    if (!entry) return;
    this.pending.delete(id);
    clearTimeout(entry.timer);
    if (error) {
      entry.reject(new Error(error));
      if (fatal) this.crash(`The engine crashed: ${error}`);
    } else {
      entry.resolve(result);
    }
  }

  async call(op, payload, timeout) {
    if (this.failed) throw new EngineUnavailable(this.failed);
    await this.ready;
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = timeout
        ? setTimeout(() => {
            this.pending.delete(id);
            reject(new Error(`The script did not finish within ${timeout / 1000} seconds and was stopped.`));
            this.crash("timeout");
          }, timeout)
        : null;
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
    return this.call("compile", request, RUN_TIMEOUT_MS);
  }

  run(request) {
    return this.call("run", request, RUN_TIMEOUT_MS);
  }
}
