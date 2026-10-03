/** One-shot, approval-based browser connection; extension lifecycle only. */
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {fileURLToPath} from 'node:url';

const state = path.join(process.env.XDG_STATE_HOME || path.join(os.homedir(), '.local/state'), 'desktop-foundation/theme');
const binding = path.join(state, 'brave-devtools.json');
const themePath = path.join(state, 'brave');
const methods = new Set(['Extensions.getExtensions', 'Extensions.loadUnpacked']);

export function endpoint(text) {
  const lines = text.trim().split('\n');
  const port = Number(lines[0]);
  if (!/^\d+$/.test(lines[0]) || port < 1 || port > 65535 ||
      !/^\/devtools\/browser\/[a-zA-Z0-9-]+$/.test(lines[1]) || lines.length !== 2) {
    throw Error('Invalid DevToolsActivePort discovery metadata');
  }
  return `ws://127.0.0.1:${port}${lines[1]}`;
}

export function request(id, method, params = {}) {
  if (!methods.has(method)) throw Error('Non-lifecycle DevTools method refused');
  return JSON.stringify({id, method, params});
}

export function installed(result, folder) {
  const matches = result.extensions.filter(e => e.path === folder);
  if (matches.length !== 1 || matches[0].name !== 'Desktop Foundation Graphite' || !matches[0].enabled) {
    throw Error('Expected one enabled Desktop Foundation theme at the stable path; install it natively first');
  }
  return matches[0];
}

export function verified(before, loaded, after, version) {
  if (loaded.id !== before.id || after.id !== before.id || after.version !== version) {
    throw Error('Theme identity/version verification failed');
  }
}

export async function reload(url, folder, version) {
  if (typeof WebSocket === 'undefined') throw Error('Node 22+ is required for native WebSocket support');
  const ws = new WebSocket(url);
  let id = 0;
  const pending = new Map();
  let failed;
  const fail = error => {
    failed = error;
    for (const p of pending.values()) p.reject(error);
    pending.clear();
  };
  ws.addEventListener('message', e => {
    try {
      const reply = JSON.parse(e.data);
      const p = pending.get(reply.id);
      if (!p) return; // No event subscriptions or tab attachments are requested.
      pending.delete(reply.id);
      reply.error ? p.reject(Error(reply.error.message)) : p.resolve(reply.result);
    } catch (error) { fail(error); }
  });
  ws.addEventListener('error', () => fail(Error('DevTools connection failed')));
  ws.addEventListener('close', () => fail(Error('DevTools connection closed')));
  function send(method, params = {}) {
    if (failed) return Promise.reject(failed);
    return new Promise((resolve, reject) => {
      pending.set(++id, {resolve, reject});
      ws.send(request(id, method, params));
    });
  }
  let timer;
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(() => reject(Error('DevTools approval/reload timed out after 45 seconds')), 45000);
  });
  const operation = async () => {
    await new Promise((resolve, reject) => {
      ws.addEventListener('open', resolve, {once: true});
      ws.addEventListener('error', () => reject(Error('DevTools connection failed')), {once: true});
      ws.addEventListener('close', () => reject(Error('DevTools connection closed')), {once: true});
    });
    const before = installed(await send('Extensions.getExtensions'), folder);
    const loaded = await send('Extensions.loadUnpacked', {path: folder});
    const after = installed(await send('Extensions.getExtensions'), folder);
    verified(before, loaded, after, version);
    console.log(`Brave theme refreshed: ${after.id}, version ${after.version}.`);
  };
  try { await Promise.race([operation(), timeout]); }
  finally { clearTimeout(timer); ws.close(); fail(Error('Helper disconnected')); }
}

async function main() {
  const args = process.argv.slice(2);
  if (args[0] === '--configure-root' && args.length === 2) {
    const root = path.resolve(args[1]);
    if (!fs.statSync(root).isDirectory() || path.basename(root) !== 'Brave-Origin-Nightly') {
      throw Error('Select the actual Brave-Origin-Nightly user-data root');
    }
    fs.mkdirSync(state, {recursive: true});
    fs.writeFileSync(binding, JSON.stringify({root}, null, 2) + '\n', {mode: 0o600});
    console.log('Theme-only reload enabled. Enable approved remote debugging in Brave UI; each connection asks approval.');
    return;
  }
  if (args[0] === '--disable' && args.length === 1) {
    fs.rmSync(binding, {force: true});
    console.log('Adapter connection disabled. Turn off remote debugging in Brave UI too.');
    return;
  }
  if (args.length) throw Error('Usage: brave-theme-reload [--configure-root /path/Brave-Origin-Nightly | --disable]');
  if (!fs.existsSync(binding)) {
    console.log(`Brave theme prepared: ${themePath}; DevTools adapter not opted in (native Load unpacked required).`);
    return;
  }
  const root = JSON.parse(fs.readFileSync(binding, 'utf8')).root;
  if (!path.isAbsolute(root) || path.basename(root) !== 'Brave-Origin-Nightly') throw Error('Invalid browser binding');
  const discovery = path.join(root, 'DevToolsActivePort');
  if (!fs.existsSync(discovery)) throw Error('Brave approved remote debugging is unavailable; enable it in brave://inspect/#remote-debugging');
  const version = JSON.parse(fs.readFileSync(path.join(themePath, 'manifest.json'), 'utf8')).version;
  console.log('Approve the native Brave DevTools connection request (theme lifecycle only).');
  await reload(endpoint(fs.readFileSync(discovery, 'utf8')), themePath, version);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(error => { console.error('Brave theme reload failed: ' + error.message); process.exitCode = 1; });
}
