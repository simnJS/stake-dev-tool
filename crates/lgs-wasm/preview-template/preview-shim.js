// Stake Dev Tool — preview shim, runs INSIDE the game iframe before the
// game's own scripts. Patches `fetch` and `XMLHttpRequest` so any call to
// `/api/rgs/<slug>/wallet/*` is answered from the WASM engine on the parent
// window instead of crossing the network.
//
// Why a shim and not the parent-side fetch override: the parent only gets
// to monkey-patch the iframe's `window.fetch` after the iframe `load`
// event, by which point the game's bundle has already cached its own
// reference to `fetch`. Even worse, many slot SDKs use XHR via axios. A
// script tag injected into the iframe's `<head>` runs before everything
// else, so the patches stick.

(function () {
  'use strict';

  function getEngine() {
    try { return window.parent && window.parent.__previewEngine; } catch { return null; }
  }
  function getBundle() {
    try { return window.parent && window.parent.__previewBundle; } catch { return null; }
  }

  async function waitForEngine(timeoutMs) {
    const deadline = Date.now() + (timeoutMs ?? 60_000);
    while (Date.now() < deadline) {
      const e = getEngine();
      if (e) return e;
      await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error('preview engine never became available on parent window');
  }

  function isRgsPath(path, slug) {
    // Replay (`/bet/replay/<slug>/`) is intentionally NOT matched: the WASM
    // dispatcher has no `replay` action, so intercepting would synthesize a
    // 500. Let these requests fall through to the real network instead.
    return path.includes(`/api/rgs/${slug}/wallet/`);
  }

  function getAction(path, slug) {
    const base = `/api/rgs/${slug}/wallet/`;
    const i = path.indexOf(base);
    if (i < 0) return null;
    return path.slice(i + base.length).split('/')[0];
  }

  function safeJsonParse(body) {
    try {
      if (typeof body === 'string') return JSON.parse(body);
      if (body instanceof Uint8Array) return JSON.parse(new TextDecoder().decode(body));
      if (body && typeof body.text === 'function') return null; // Blob / FormData → skip
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
        const mode = (body && body.mode) || 'base';
        // Lazy-load: runtime.js only pre-loads the initial mode at boot.
        // First play in a mode pays a one-shot fetch + decompress here.
        // Idempotent — `loadMode` short-circuits if already loaded.
        await engine.loadMode(mode);
        const amount = BigInt((body && body.amount) || 0);
        return engine.play(mode, amount);
      }
      case 'end-round':
        return engine.endRound();
      default:
        throw new Error('unknown RGS action: ' + action);
    }
  }

  function notifyFirstCall() {
    if (window.__sdtFirstCallFired) return;
    window.__sdtFirstCallFired = true;
    try {
      const cb = window.parent.__onFirstRgsCall;
      if (typeof cb === 'function') cb();
    } catch {}
  }

  // ---- fetch patch ----
  const realFetch = window.fetch ? window.fetch.bind(window) : null;
  window.fetch = async function patchedFetch(input, init) {
    try {
      const bundle = getBundle();
      if (!bundle) return realFetch ? realFetch(input, init) : Promise.reject(new Error('no bundle'));
      const slug = bundle.gameSlug;
      const url = typeof input === 'string' ? input : input.url;
      const u = new URL(url, location.href);
      if (!isRgsPath(u.pathname, slug)) {
        return realFetch(input, init);
      }
      const engine = await waitForEngine();
      const action = getAction(u.pathname, slug);
      const body = init && init.body ? safeJsonParse(init.body) : null;
      const json = await dispatch(engine, action, body);
      notifyFirstCall();
      return new Response(JSON.stringify(json), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      });
    } catch (e) {
      console.error('[preview-shim] fetch dispatch failed:', e);
      return new Response(JSON.stringify({ error: String(e) }), {
        status: 500,
        headers: { 'content-type': 'application/json' },
      });
    }
  };

  // ---- XMLHttpRequest patch ----
  const RealXHR = window.XMLHttpRequest;
  function PatchedXHR() {
    const xhr = new RealXHR();
    let _url = '';
    let _method = 'GET';
    let _intercept = false;

    const _open = xhr.open;
    xhr.open = function (method, url) {
      _method = method;
      _url = url;
      try {
        const bundle = getBundle();
        const u = new URL(url, location.href);
        _intercept = bundle && isRgsPath(u.pathname, bundle.gameSlug);
      } catch {
        _intercept = false;
      }
      return _open.apply(xhr, arguments);
    };

    const _send = xhr.send;
    xhr.send = function (body) {
      if (!_intercept) return _send.apply(xhr, arguments);
      (async () => {
        try {
          const bundle = getBundle();
          const slug = bundle.gameSlug;
          const engine = await waitForEngine();
          const u = new URL(_url, location.href);
          const action = getAction(u.pathname, slug);
          const parsed = body ? safeJsonParse(body) : null;
          const json = await dispatch(engine, action, parsed);
          notifyFirstCall();
          const text = JSON.stringify(json);
          // Synthesize an XHR response. Using defineProperty because the
          // real XHR backing fields are read-only.
          Object.defineProperty(xhr, 'readyState', { value: 4, configurable: true });
          Object.defineProperty(xhr, 'status', { value: 200, configurable: true });
          Object.defineProperty(xhr, 'statusText', { value: 'OK', configurable: true });
          Object.defineProperty(xhr, 'response', { value: text, configurable: true });
          Object.defineProperty(xhr, 'responseText', { value: text, configurable: true });
          Object.defineProperty(xhr, 'responseURL', { value: _url, configurable: true });
          Object.defineProperty(xhr, 'getAllResponseHeaders', {
            value: () => 'content-type: application/json\r\n',
            configurable: true,
          });
          Object.defineProperty(xhr, 'getResponseHeader', {
            value: (h) => (h.toLowerCase() === 'content-type' ? 'application/json' : null),
            configurable: true,
          });
          xhr.dispatchEvent(new Event('readystatechange'));
          xhr.dispatchEvent(new Event('load'));
          xhr.dispatchEvent(new Event('loadend'));
        } catch (e) {
          console.error('[preview-shim] xhr dispatch failed:', e);
          Object.defineProperty(xhr, 'readyState', { value: 4, configurable: true });
          Object.defineProperty(xhr, 'status', { value: 500, configurable: true });
          xhr.dispatchEvent(new Event('error'));
          xhr.dispatchEvent(new Event('loadend'));
        }
      })();
    };
    return xhr;
  }
  PatchedXHR.prototype = RealXHR.prototype;
  window.XMLHttpRequest = PatchedXHR;
})();
