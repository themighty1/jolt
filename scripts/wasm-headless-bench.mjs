#!/usr/bin/env node
// Run a Jolt wasm example in headless Chrome and print just the bench result(s).
//
// Usage:
//   scripts/wasm-headless-bench.mjs <example> [--input X] [--query X] [--size N] [--threads N] [--timeout SEC]
//   example: integer-check | json-parse | json-query
//
// Env: CHROME=/path/to/chrome to override the binary (default: google-chrome).
//
// Output: lines starting with BENCH_READY: / BENCH_RESULT: / BENCH_ERROR: / BENCH_DONE.

import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, createConnection } from 'node:net';
import { setTimeout as sleep } from 'node:timers/promises';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, '..');

const EXAMPLES = {
  'integer-check': {
    dir: 'examples/integer-check-wasm',
    description: 'Prove a string input parses as an integer > 700.',
    options: [
      { flag: '--input', value: 'STRING', help: 'string to check (default: 701)' },
    ],
  },
  'json-parse': {
    dir: 'examples/json-parse-wasm',
    description: 'Prove a dot-path query over a fixed private JSON (full validation, ZK).',
    options: [
      { flag: '--query', value: 'PATH', help: 'dot-path query (default: records.5.amount)' },
    ],
  },
  'json-query': {
    dir: 'examples/json-query-wasm',
    description: 'Prove a dot-path query without full JSON validation (lazy, ZK).',
    options: [
      { flag: '--size', value: '1|2|4|8|16|32', help: 'JSON size in KB (default: 4)' },
      { flag: '--query', value: 'PATH', help: 'dot-path query (default: records.<last>.amount)' },
    ],
  },
};

const COMMON_OPTIONS = [
  { flag: '--threads', value: 'N', help: 'thread count (default: navigator.hardwareConcurrency)' },
  { flag: '--timeout', value: 'SEC', help: 'overall deadline before runner aborts (default: 600)' },
];

function formatHelp() {
  const pad = (s, n) => s + ' '.repeat(Math.max(0, n - s.length));
  const fmtOpt = (o) => `      ${pad(`${o.flag} ${o.value}`, 22)} ${o.help}`;
  let s = `Usage: wasm-headless-bench.mjs <example> [options]\n\n`;
  s += `Runs a Jolt wasm example in headless Chrome and prints BENCH_* lines to stdout.\n\n`;
  s += `Examples:\n`;
  for (const [name, e] of Object.entries(EXAMPLES)) {
    s += `\n  ${name}\n    ${e.description}\n`;
    if (e.options.length) {
      s += `    Options:\n`;
      for (const o of e.options) s += fmtOpt(o) + '\n';
    }
  }
  s += `\nCommon options (all examples):\n`;
  for (const o of COMMON_OPTIONS) s += fmtOpt(o) + '\n';
  s += `\nEnv:\n      CHROME=/path/to/chrome   override the browser binary (default: google-chrome)\n`;
  return s;
}

const args = process.argv.slice(2);
if (args.length === 0 || args[0] === '-h' || args[0] === '--help') {
  process.stdout.write(formatHelp());
  process.exit(args.length === 0 ? 1 : 0);
}
const example = args[0];
if (!EXAMPLES[example]) {
  console.error(`Unknown example: ${example}. Try: ${Object.keys(EXAMPLES).join(', ')}`);
  process.exit(1);
}
const opts = { timeout: '600' };
for (let i = 1; i < args.length; i += 2) {
  const key = args[i].replace(/^--/, '');
  opts[key] = args[i + 1];
}

const exampleDir = join(REPO_ROOT, EXAMPLES[example].dir);
if (!existsSync(join(exampleDir, 'pkg'))) {
  console.error(`Missing ${exampleDir}/pkg — build the wasm first (e.g. wasm-pack build).`);
  process.exit(1);
}

function pickFreePort() {
  return new Promise((resolve, reject) => {
    const srv = createServer();
    srv.unref();
    srv.on('error', reject);
    srv.listen(0, '127.0.0.1', () => {
      const port = srv.address().port;
      srv.close(() => resolve(port));
    });
  });
}

function waitForTcp(port, host, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    const tryOnce = () => {
      const s = createConnection({ port, host });
      s.once('connect', () => { s.end(); resolve(); });
      s.once('error', () => {
        s.destroy();
        if (Date.now() > deadline) reject(new Error(`tcp ${host}:${port} not ready`));
        else setTimeout(tryOnce, 100);
      });
    };
    tryOnce();
  });
}

const userDataDir = mkdtempSync(join(tmpdir(), 'jolt-headless-'));
let serve, chrome, ws;
let cleaned = false;
function cleanup(code) {
  if (cleaned) return;
  cleaned = true;
  try { ws?.close(); } catch {}
  try { chrome?.kill('SIGKILL'); } catch {}
  try { serve?.kill('SIGKILL'); } catch {}
  try { rmSync(userDataDir, { recursive: true, force: true }); } catch {}
  process.exit(code);
}
process.on('SIGINT', () => cleanup(130));
process.on('SIGTERM', () => cleanup(143));

async function main() {
  const httpPort = await pickFreePort();
  serve = spawn('python3', ['serve.py', String(httpPort)], {
    cwd: exampleDir,
    stdio: ['ignore', 'ignore', 'ignore'],
  });
  serve.on('exit', (code) => {
    if (!cleaned) {
      console.error(`serve.py exited prematurely (code ${code})`);
      cleanup(1);
    }
  });
  await waitForTcp(httpPort, '127.0.0.1', 10_000);

  const params = new URLSearchParams({ headless: '1' });
  for (const k of ['input', 'query', 'size', 'threads']) {
    if (opts[k] !== undefined) params.set(k, opts[k]);
  }
  const targetUrl = `http://localhost:${httpPort}/?${params}`;

  const chromeBin = process.env.CHROME || 'google-chrome';
  chrome = spawn(chromeBin, [
    '--headless=new',
    '--disable-gpu',
    '--no-sandbox',
    '--disable-dev-shm-usage',
    `--user-data-dir=${userDataDir}`,
    '--remote-debugging-port=0',
    '--enable-features=SharedArrayBuffer',
    'about:blank',
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
  chrome.on('exit', (code) => {
    if (!cleaned) {
      console.error(`chrome exited prematurely (code ${code})`);
      cleanup(1);
    }
  });

  const portFile = join(userDataDir, 'DevToolsActivePort');
  const startupDeadline = Date.now() + 30_000;
  while (!existsSync(portFile)) {
    if (Date.now() > startupDeadline) {
      console.error('chrome did not expose DevToolsActivePort within 30s');
      cleanup(1);
    }
    await sleep(100);
  }
  const debugPort = Number(readFileSync(portFile, 'utf8').split('\n')[0]);

  const versionRes = await fetch(`http://127.0.0.1:${debugPort}/json/version`);
  const browserWsUrl = (await versionRes.json()).webSocketDebuggerUrl;

  ws = new WebSocket(browserWsUrl);
  await new Promise((res, rej) => {
    ws.addEventListener('open', res, { once: true });
    ws.addEventListener('error', rej, { once: true });
  });

  let msgId = 0;
  const pending = new Map();
  let sessionId = null;

  ws.addEventListener('message', (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id != null && pending.has(m.id)) {
      const { resolve, reject } = pending.get(m.id);
      pending.delete(m.id);
      if (m.error) reject(new Error(m.error.message));
      else resolve(m.result);
      return;
    }
    if (m.method === 'Runtime.consoleAPICalled' && m.sessionId === sessionId) {
      const text = m.params.args
        .map((a) => (a.value !== undefined ? a.value : a.description ?? ''))
        .join(' ');
      if (
        text.startsWith('BENCH_RESULT:') ||
        text.startsWith('BENCH_READY:') ||
        text.startsWith('BENCH_ERROR:')
      ) {
        console.log(text);
      } else if (text === 'BENCH_DONE') {
        console.log(text);
        cleanup(0);
      }
    } else if (m.method === 'Runtime.exceptionThrown' && m.sessionId === sessionId) {
      console.error('PAGE_EXCEPTION:', m.params.exceptionDetails?.exception?.description ?? m.params.exceptionDetails?.text);
    }
  });

  function call(method, params = {}, sid) {
    return new Promise((resolve, reject) => {
      const id = ++msgId;
      pending.set(id, { resolve, reject });
      const msg = { id, method, params };
      if (sid) msg.sessionId = sid;
      ws.send(JSON.stringify(msg));
    });
  }

  const { targetId } = await call('Target.createTarget', { url: 'about:blank' });
  const attached = await call('Target.attachToTarget', { targetId, flatten: true });
  sessionId = attached.sessionId;

  await call('Runtime.enable', {}, sessionId);
  await call('Page.enable', {}, sessionId);
  await call('Page.navigate', { url: targetUrl }, sessionId);

  const timeoutMs = Number(opts.timeout) * 1000;
  setTimeout(() => {
    console.error(`Timed out after ${opts.timeout}s without BENCH_DONE`);
    cleanup(2);
  }, timeoutMs);
}

main().catch((e) => {
  console.error('runner error:', e?.stack || e);
  cleanup(1);
});
