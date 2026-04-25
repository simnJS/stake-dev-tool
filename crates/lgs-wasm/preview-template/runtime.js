// Stake Dev Tool — preview runtime.
//
// Boots the WASM engine, then loads the game in an iframe and monkey-patches
// the iframe's `fetch` so any call to the RGS contract is answered from the
// in-browser engine instead of crossing the network. Same-origin iframe →
// the patch is allowed by the browser.

import init, { PreviewEngine } from './lgs_wasm.js';

const SPLASH = document.getElementById('splash');
const STEP = document.getElementById('splash-step');
const BADGE = document.getElementById('badge-game');
const FRAME = document.getElementById('game-frame');

function setStep(s) {
  STEP.textContent = s;
}
function fail(msg) {
  STEP.classList.add('err');
  STEP.textContent = msg;
  // Hide spinner on error.
  const spinner = SPLASH.querySelector('.spinner');
  if (spinner) spinner.style.display = 'none';
}

async function boot() {
  let bundle;
  try {
    setStep('Loading bundle manifest…');
    const res = await fetch('./bundle.json', { cache: 'no-store' });
    if (!res.ok) throw new Error(`bundle.json: ${res.status}`);
    bundle = await res.json();
  } catch (e) {
    fail(`Could not load bundle.json — ${e.message ?? e}`);
    return;
  }

  BADGE.textContent = bundle.gameSlug ?? bundle.game ?? '?';

  // ===================================================================
  // Race the iframe load against the WASM boot. The game's asset pipeline
  // (bundle JS, sprite atlases, sounds — often 10-50 MB total) usually
  // dominates wall-clock time on first load. By starting the iframe NOW
  // instead of after `await init()`, those network requests overlap with
  // the WASM compile + manifest fetch.
  //
  // Safety: the iframe shim's `waitForEngine` polls until
  // `window.parent.__previewEngine` is set, so even if the game makes its
  // first /authenticate before WASM is ready, it just waits a few hundred
  // ms instead of failing.
  // ===================================================================

  const rgsRelative = `${location.pathname}api/rgs/${bundle.gameSlug}`;
  const gameSrcUrl = new URL(bundle.gameEntry ?? './game/index.html', location.href);
  const params = new URLSearchParams({
    sessionID: 'preview-session',
    rgs_url: location.host + rgsRelative,
    lang: bundle.lang ?? 'en',
    currency: bundle.currency ?? 'USD',
    device: bundle.device ?? 'desktop',
    social: 'false',
  });
  if (bundle.extraParams) {
    for (const [k, v] of Object.entries(bundle.extraParams)) params.set(k, String(v));
  }
  history.replaceState(null, '', `?${params.toString()}`);
  gameSrcUrl.search = params.toString();

  // Splash hides on first RGS call (the moment we know the game has
  // bootstrapped enough to make a network request). 30s timeout is a
  // safety net.
  let splashHidden = false;
  function maybeHideSplash() {
    if (splashHidden) return;
    splashHidden = true;
    SPLASH.classList.add('gone');
  }
  const fallbackTimer = window.setTimeout(() => {
    if (!splashHidden) {
      console.warn('No RGS call seen yet; hiding splash on timeout.');
      maybeHideSplash();
    }
  }, 30_000);

  FRAME.addEventListener('load', () => {
    // Fallback fetch patch — the in-iframe shim usually catches everything,
    // but install this one too for any window.fetch references the game
    // might have captured pre-shim.
    try {
      patchFetch(FRAME.contentWindow, bundle, () => {
        clearTimeout(fallbackTimer);
        maybeHideSplash();
      });
    } catch (e) {
      console.error('fetch patch failed', e);
    }
    setStep('Game booting…');
  });
  setStep('Booting game…');
  FRAME.src = gameSrcUrl.toString();

  // Start WASM bootstrap in the background. The shim awaits
  // `window.parent.__previewEngine` which we set when ready below.
  try {
    setStep('Instantiating WASM (parallel)…');
    await init();
  } catch (e) {
    fail(`WASM instantiation failed — ${e.message ?? e}`);
    return;
  }

  /** @type {PreviewEngine} */
  let engine;
  try {
    const mathBase = bundle.mathBaseUrl ?? './math';
    engine = new PreviewEngine(bundle.gameSlug, mathBase);
    const cacheBust = `?v=${Date.now()}`;
    try {
      await engine.loadManifest(`${mathBase}/math-manifest.json${cacheBust}`);
    } catch (e) {
      console.warn('No chunk manifest, using direct fetch:', e);
    }
  } catch (e) {
    fail(`Math load failed — ${e.message ?? e}`);
    return;
  }

  // Engine ready — the shim's `waitForEngine` polls picks it up on its
  // next tick.
  window.__previewEngine = engine;
  window.__previewBundle = bundle;
}

/**
 * Replace the iframe's `fetch` with a wrapper that routes RGS calls to the
 * WASM engine and proxies everything else to the real network. The pattern
 * we match is `/api/rgs/<slug>/wallet/{authenticate,balance,play,end-round}`.
 *
 * `onFirstRgsCall` fires the first time the iframe makes any matching
 * request — used to hide the splash once the game has actually started
 * talking to the RGS rather than just rendered its document.
 */
function patchFetch(win, bundle, onFirstRgsCall) {
  if (!win) return;
  const realFetch = win.fetch.bind(win);
  const slug = bundle.gameSlug;
  let firstCallFired = false;

  async function waitForEngine() {
    const deadline = Date.now() + 60_000;
    while (Date.now() < deadline) {
      const e = win.parent && win.parent.__previewEngine;
      if (e) return e;
      await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error('preview engine never became available');
  }

  win.fetch = async function patchedFetch(input, init) {
    try {
      const url = typeof input === 'string' ? input : input.url;
      const u = new URL(url, win.location.href);
      const path = u.pathname;
      const rgsBase = `/api/rgs/${slug}/wallet/`;
      // Replay (`/bet/replay/<slug>/`) is intentionally NOT matched: the WASM
      // dispatcher has no `replay` action, so intercepting would synthesize a
      // 500. Letting these requests fall through to the real network gives the
      // game its normal "replay unavailable" path.
      if (path.includes(rgsBase)) {
        if (!firstCallFired) {
          firstCallFired = true;
          try { onFirstRgsCall?.(); } catch {}
        }
        const engine = await waitForEngine();
        const body = init?.body ? safeJsonParse(init.body) : null;
        const action = path.slice(path.indexOf(rgsBase) + rgsBase.length).split('/')[0];
        const json = await dispatch(engine, action, body);
        return new Response(JSON.stringify(json), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        });
      }
    } catch (e) {
      console.error('preview fetch dispatch failed', e);
      return new Response(JSON.stringify({ error: String(e) }), {
        status: 500,
        headers: { 'content-type': 'application/json' },
      });
    }
    return realFetch(input, init);
  };
}

function safeJsonParse(body) {
  try {
    if (typeof body === 'string') return JSON.parse(body);
    if (body instanceof Uint8Array) return JSON.parse(new TextDecoder().decode(body));
    return null;
  } catch {
    return null;
  }
}

async function dispatch(engine, action, body) {
  switch (action) {
    case 'authenticate':
      return engine.authenticate();
    case 'balance':
      return engine.balance();
    case 'play': {
      const mode = body?.mode ?? 'base';
      const amount = BigInt(body?.amount ?? 0);
      await engine.loadMode(mode);
      return engine.play(mode, amount);
    }
    case 'end-round':
      return engine.endRound();
    default:
      throw new Error(`unknown RGS action: ${action}`);
  }
}

boot();
