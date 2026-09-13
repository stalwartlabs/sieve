const SHARE_PARAM = "w";
const FORMAT = 1;
export const LONG_LINK = 8000;

function toBase64Url(bytes) {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function fromBase64Url(text) {
  const base64 = text.replace(/-/g, "+").replace(/_/g, "/");
  const binary = atob(base64 + "===".slice((base64.length + 3) % 4));
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

async function pipe(bytes, stream) {
  const response = new Response(new Blob([bytes]).stream().pipeThrough(stream));
  return new Uint8Array(await response.arrayBuffer());
}

export function settingsDiff(settings, defaults) {
  const diff = {};
  for (const [key, value] of Object.entries(settings || {})) {
    if (JSON.stringify(value) !== JSON.stringify(defaults[key])) diff[key] = value;
  }
  return diff;
}

export function portable(workspace, defaults, { includeMessages = true } = {}) {
  return {
    v: FORMAT,
    name: workspace.name,
    scripts: workspace.scripts.map(({ name, source }) => ({ name, source })),
    messages: includeMessages ? workspace.messages.map(({ name, source }) => ({ name, source })) : [],
    settings: settingsDiff(workspace.settings, defaults),
  };
}

export async function encodeShareUrl(data) {
  const json = new TextEncoder().encode(JSON.stringify(data));
  const packed = await pipe(json, new CompressionStream("deflate-raw"));
  const url = new URL(location.href);
  url.search = "";
  url.hash = `${SHARE_PARAM}=${toBase64Url(packed)}`;
  return url.href;
}

export function hasSharedWorkspace() {
  return new URLSearchParams(location.hash.slice(1)).has(SHARE_PARAM);
}

export async function decodeSharedWorkspace() {
  const value = new URLSearchParams(location.hash.slice(1)).get(SHARE_PARAM);
  if (!value) return null;
  const json = await pipe(fromBase64Url(value), new DecompressionStream("deflate-raw"));
  return validatePortable(JSON.parse(new TextDecoder().decode(json)));
}

export function clearShareHash() {
  history.replaceState(null, "", location.pathname + location.search);
}

export function validatePortable(data) {
  if (!data || typeof data !== "object" || !Array.isArray(data.scripts)) {
    throw new Error("This is not a Sievepad workspace.");
  }
  const clean = (items) =>
    (Array.isArray(items) ? items : [])
      .filter((item) => item && typeof item.source === "string")
      .map((item, index) => ({ name: String(item.name || `item-${index + 1}`).slice(0, 80), source: item.source }));
  const scripts = clean(data.scripts);
  if (scripts.length === 0) throw new Error("The workspace has no scripts.");
  return {
    name: String(data.name || "Shared workspace").slice(0, 80),
    scripts,
    messages: clean(data.messages),
    settings: data.settings && typeof data.settings === "object" ? data.settings : {},
  };
}
