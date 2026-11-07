// The browser half of the web launcher (src/glue.rs binds these functions): downloads run
// through fetch, files through the File System Access API, sha1 through Web Crypto. Mojang's
// piston-meta and piston-data and GitHub's API allow cross-origin requests; the folder the
// player picks is theirs alone — nothing is uploaded anywhere.
//
// The folder is kept in this module (a `let`, not exported: only the functions are bound).
// Every async function returns a Promise that rejects with an Error on failure.

let folder = null;

// Whether the browser has the File System Access API (Chromium; Firefox and Safari offer the
// downloads and the releases without it).
export function canPickFolder() {
  return typeof window.showDirectoryPicker === "function";
}

// The folder picker must run inside the click's transient activation: called straight from
// the button, this task is still inside it. "ok", "cancelled" or "unsupported".
export async function pickFolder() {
  if (!canPickFolder()) return "unsupported";
  try {
    folder = await window.showDirectoryPicker({ mode: "readwrite" });
    return "ok";
  } catch (e) {
    if (e && e.name === "AbortError") return "cancelled";
    throw e;
  }
}

export function hasFolder() {
  return folder !== null;
}

export function folderName() {
  return folder ? folder.name : "";
}

// The handle of `path` (a/b/c.txt) inside the folder, walking into (with `create`) or through
// the directories named before the file.
async function fileHandle(path, create) {
  let dir = folder;
  const parts = path.split("/").filter((part) => part.length > 0);
  const name = parts.pop();
  for (const part of parts) {
    dir = await dir.getDirectoryHandle(part, { create: create });
  }
  return dir.getFileHandle(name, { create: create });
}

// The file's sha1 as Web Crypto computes it, or null when there is no folder or no such file.
export async function folderSha1(path) {
  if (!folder) return null;
  try {
    const file = await (await fileHandle(path, false)).getFile();
    const digest = await crypto.subtle.digest("SHA-1", await file.arrayBuffer());
    return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
  } catch (e) {
    if (e && (e.name === "TypeError" || e.name === "NotFoundError")) return null;
    throw e;
  }
}

// A small file's bytes (the version JSON, to plan the rest from).
export async function readFile(path) {
  const file = await (await fileHandle(path, false)).getFile();
  return new Uint8Array(await file.arrayBuffer());
}

// Writes the file whole: the writable stream takes its time in a temporary file, and the
// existing one is replaced only when it closes — a download stopped half way never leaves a
// partial file behind.
export async function writeFile(path, bytes) {
  const handle = await fileHandle(path, true);
  const writable = await handle.createWritable();
  await writable.write(bytes);
  await writable.close();
}

// The url's bytes, with `onProgress(loaded, total)` as they arrive.
export async function fetchBytes(url, onProgress) {
  const response = await fetch(url);
  if (!response.ok) throw new Error("HTTP " + response.status);
  const total = Number(response.headers.get("content-length") || 0);
  if (!response.body || !total || !onProgress) {
    return new Uint8Array(await response.arrayBuffer());
  }
  const reader = response.body.getReader();
  const parts = [];
  let loaded = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    parts.push(value);
    loaded += value.length;
    onProgress(loaded, total);
  }
  const bytes = new Uint8Array(loaded);
  let at = 0;
  for (const part of parts) {
    bytes.set(part, at);
    at += part.length;
  }
  return bytes;
}

// A text resource (GitHub's API), with the API's media type.
export async function fetchText(url) {
  const response = await fetch(url, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!response.ok) {
    const error = new Error("HTTP " + response.status);
    error.status = response.status;
    throw error;
  }
  return response.text();
}

export function openUrl(url) {
  window.open(url, "_blank", "noopener");
}

export function userAgent() {
  return navigator.userAgent;
}

// The browser's preferred languages ("en-US,id,en"), for the web launcher's language.
export function languages() {
  return (navigator.languages || [navigator.language || "en"]).join(",");
}
