const DB_NAME = "sievepad";
const DB_VERSION = 1;
const STORE = "workspaces";
const PREFS_KEY = "sievepad.prefs";
const FALLBACK_KEY = "sievepad.workspaces";
const WELCOME_KEY = "sievepad.welcome-dismissed";

function openDb() {
  return new Promise((resolve, reject) => {
    if (!("indexedDB" in self)) {
      reject(new Error("IndexedDB unavailable"));
      return;
    }
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      request.result.createObjectStore(STORE, { keyPath: "id" });
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

function wrap(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

class IdbStore {
  constructor(db) {
    this.db = db;
  }

  tx(mode) {
    return this.db.transaction(STORE, mode).objectStore(STORE);
  }

  list() {
    return wrap(this.tx("readonly").getAll());
  }

  get(id) {
    return wrap(this.tx("readonly").get(id));
  }

  put(workspace) {
    return wrap(this.tx("readwrite").put(workspace));
  }

  delete(id) {
    return wrap(this.tx("readwrite").delete(id));
  }
}

class MemoryStore {
  constructor() {
    this.items = new Map();
    try {
      for (const ws of JSON.parse(localStorage.getItem(FALLBACK_KEY) || "[]")) {
        this.items.set(ws.id, ws);
      }
    } catch (_) {
      this.items.clear();
    }
  }

  persist() {
    try {
      localStorage.setItem(FALLBACK_KEY, JSON.stringify([...this.items.values()]));
    } catch (_) {
      return;
    }
  }

  async list() {
    return [...this.items.values()];
  }

  async get(id) {
    return this.items.get(id);
  }

  async put(workspace) {
    this.items.set(workspace.id, workspace);
    this.persist();
  }

  async delete(id) {
    this.items.delete(id);
    this.persist();
  }
}

export async function openStore() {
  try {
    return new IdbStore(await openDb());
  } catch (_) {
    return new MemoryStore();
  }
}

export function loadPrefs() {
  try {
    return JSON.parse(localStorage.getItem(PREFS_KEY) || "{}");
  } catch (_) {
    return {};
  }
}

export function savePrefs(prefs) {
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify({ ...loadPrefs(), ...prefs }));
  } catch (_) {
    return;
  }
}

export function isWelcomeDismissed() {
  try {
    return localStorage.getItem(WELCOME_KEY) !== null;
  } catch (_) {
    return false;
  }
}

export function dismissWelcome() {
  try {
    localStorage.setItem(WELCOME_KEY, new Date().toISOString());
  } catch (_) {
    return;
  }
}

export function newId() {
  if (self.crypto && crypto.randomUUID) return crypto.randomUUID();
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}
