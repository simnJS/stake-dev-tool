<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import Picker from '$lib/Picker.svelte';
  import {
    LANGUAGES,
    CURRENCIES,
    API_MULTIPLIER,
    settingsHttp,
    type ResolutionPreset
  } from '$lib/api';

  // Stake social-mode currencies (XGC = Gold Coin, XSC = Stake Cash) only
  // make sense when social=true; hide them otherwise.
  const SOCIAL_CURRENCIES = new Set(['XGC', 'XSC']);
  const realCurrencies = CURRENCIES.filter((c) => !SOCIAL_CURRENCIES.has(c.code));
  const socialCurrencies = CURRENCIES.filter((c) => SOCIAL_CURRENCIES.has(c.code));

  type FrameState = {
    res: ResolutionPreset;
    sessionId: string;
    src: string | null;
    muted: boolean;
  };

  let gameUrl = $state('');
  let gameSlug = $state('');

  let balance = $state(10000);
  let currency = $state<string>('USD');
  let language = $state<string>('en');
  let social = $state(false);
  let device = $state('desktop');

  // Available currencies depend on social mode: XGC/XSC are social-only.
  const availableCurrencies = $derived(social ? socialCurrencies : realCurrencies);

  // Auto-switch currency when toggling social mode in/out of valid set.
  function toggleSocial() {
    social = !social;
    if (social) {
      if (!SOCIAL_CURRENCIES.has(currency)) currency = 'XGC';
    } else {
      if (SOCIAL_CURRENCIES.has(currency)) currency = 'USD';
    }
  }

  let allResolutions = $state<ResolutionPreset[]>([]);
  let frames = $state<FrameState[]>([]);
  let showManage = $state(false);
  let newCustomLabel = $state('');
  let newCustomWidth = $state(800);
  let newCustomHeight = $state(450);

  function rebuildFramesFromResolutions(prev: FrameState[] = []) {
    const enabled = allResolutions.filter((r) => r.enabled);
    // Preserve existing frame state for resolutions still enabled.
    const byId = new Map(prev.map((f) => [f.res.id, f]));
    frames = enabled.map((res) => {
      const existing = byId.get(res.id);
      if (existing) {
        // Update dimensions if user resized a custom one
        existing.res = res;
        return existing;
      }
      return { res, sessionId: crypto.randomUUID(), src: null, muted: true };
    });
  }

  let busy = $state(false);
  let error = $state<string | null>(null);
  let info = $state<string | null>(null);

  // We're served by the LGS itself, so APIs are same-origin.
  const lgsBase = `${location.origin}`;
  // For rgs_url passed to the game we need host:port (no scheme, the game adds https).
  const lgsHostPort = location.host;

  onMount(async () => {
    const params = page.url.searchParams;
    gameUrl = params.get('gameUrl') ?? '';
    gameSlug = params.get('gameSlug') ?? '';
    if (!gameUrl || !gameSlug) {
      error = 'Missing game URL or slug. Open this page from the desktop launcher.';
      return;
    }
    try {
      const s = await settingsHttp.get();
      allResolutions = s.resolutions;
      rebuildFramesFromResolutions();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      return;
    }
    await reloadAll();
  });

  // ---- Resolution management ----

  async function toggleResolution(id: string, enabled: boolean) {
    busy = true;
    try {
      const s = await settingsHttp.toggle(id, enabled);
      allResolutions = s.resolutions;
      rebuildFramesFromResolutions(frames);
      // load any newly enabled frames
      const newlyEnabled = frames.filter((f) => f.src === null);
      for (const f of newlyEnabled) {
        await reloadFrame(f);
        await new Promise((r) => setTimeout(r, 600));
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function addCustomResolution() {
    const label = newCustomLabel.trim();
    if (!label || newCustomWidth <= 0 || newCustomHeight <= 0) {
      error = 'Label, width and height required.';
      return;
    }
    busy = true;
    try {
      const s = await settingsHttp.addCustom(label, newCustomWidth, newCustomHeight);
      allResolutions = s.resolutions;
      rebuildFramesFromResolutions(frames);
      newCustomLabel = '';
      const last = frames[frames.length - 1];
      if (last && last.src === null) await reloadFrame(last);
      info = `Added "${label}" (${newCustomWidth}×${newCustomHeight}).`;
      setTimeout(() => (info = null), 2000);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function deleteCustomResolution(id: string) {
    if (!confirm('Delete this custom resolution?')) return;
    busy = true;
    try {
      const s = await settingsHttp.deleteCustom(id);
      allResolutions = s.resolutions;
      rebuildFramesFromResolutions(frames);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function buildGameUrlFor(sessionId: string): string {
    if (!gameUrl) return '';
    const rgsUrl = `${lgsHostPort}/api/rgs/${gameSlug}`;
    const u = new URL(gameUrl);
    u.searchParams.set('sessionID', sessionId);
    u.searchParams.set('rgs_url', rgsUrl);
    u.searchParams.set('lang', language);
    u.searchParams.set('currency', currency);
    u.searchParams.set('device', device);
    u.searchParams.set('social', social ? 'true' : 'false');
    return u.toString();
  }

  async function prepareSession(sessionId: string) {
    const res = await fetch(`${lgsBase}/api/admin/sessions/prepare`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        sessionId,
        gameSlug,
        balance: Math.round(balance * API_MULTIPLIER),
        currency,
        language
      })
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`prepare failed: ${res.status} ${text}`);
    }
  }

  async function reloadFrame(frame: FrameState, regenerateSession = true) {
    if (regenerateSession) frame.sessionId = crypto.randomUUID();
    await prepareSession(frame.sessionId);
    frame.src = buildGameUrlFor(frame.sessionId);
  }

  async function reloadAll() {
    busy = true;
    error = null;
    try {
      // Clear all iframes first, then load one at a time with a delay so each
      // game gets time to initialize its WebGL context without racing the
      // others (PixiJS shader compilation crashes under concurrent load).
      frames.forEach((f) => (f.src = null));
      for (const f of frames) {
        await reloadFrame(f);
        await new Promise((r) => setTimeout(r, 800));
      }
      info = `Reloaded ${frames.length} frames · balance=${balance} ${currency}`;
      setTimeout(() => (info = null), 2500);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function reloadOne(frame: FrameState) {
    busy = true;
    error = null;
    try {
      await reloadFrame(frame);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function openInBrowser(frame: FrameState) {
    if (!frame.src) return;
    window.open(frame.src, '_blank', 'noopener,noreferrer');
  }

  function toggleMute(frame: FrameState) {
    frame.muted = !frame.muted;
  }

  function muteAll() {
    frames.forEach((f) => (f.muted = true));
  }
  function unmuteAll() {
    frames.forEach((f) => (f.muted = false));
  }
</script>

<svelte:head>
  <title>Stake Dev Tool · Test view</title>
</svelte:head>

<div class="flex h-screen overflow-hidden bg-zinc-950 text-zinc-100">
  <!-- Sidebar -->
  <aside class="flex w-96 flex-shrink-0 flex-col border-r border-zinc-800/60 bg-zinc-900/40 p-5">
    <div class="mb-4">
      <div class="text-[10px] font-medium uppercase tracking-wider text-zinc-500">Game</div>
      <div class="mt-0.5 truncate text-sm font-semibold text-zinc-100">{gameSlug || '—'}</div>
    </div>

    <div class="mb-4 space-y-3">
      <div>
        <label for="initial-balance" class="mb-1 block text-[10px] font-medium uppercase tracking-wider text-zinc-500">
          Initial balance
        </label>
        <div class="flex gap-1.5">
          <input
            id="initial-balance"
            name="initial-balance"
            type="number"
            bind:value={balance}
            min="0"
            step="100"
            class="flex-1 rounded-md border border-zinc-800 bg-zinc-950/60 px-2.5 py-1.5 font-mono text-sm focus:border-emerald-500/50 focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
          <div class="w-36 flex-shrink-0">
            <Picker
              items={availableCurrencies}
              value={currency}
              onSelect={(c) => (currency = c)}
            />
          </div>
        </div>
      </div>

      <div class="grid grid-cols-2 gap-2">
        <div>
          <div class="mb-1 block text-[10px] font-medium uppercase tracking-wider text-zinc-500">
            Lang
          </div>
          <Picker items={LANGUAGES} value={language} onSelect={(l) => (language = l)} />
        </div>
        <div>
          <label for="device-select" class="mb-1 block text-[10px] font-medium uppercase tracking-wider text-zinc-500">
            Device
          </label>
          <select
            id="device-select"
            name="device-select"
            bind:value={device}
            class="w-full rounded-md border border-zinc-800 bg-zinc-950/60 px-2 py-1.5 font-mono text-xs focus:border-emerald-500/50 focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          >
            <option value="desktop">desktop</option>
            <option value="mobile">mobile</option>
          </select>
        </div>
      </div>

      <button
        type="button"
        onclick={toggleSocial}
        class="flex w-full cursor-pointer items-center justify-between gap-2 rounded-md border bg-zinc-950/40 px-2.5 py-1.5 text-xs transition {social
          ? 'border-amber-700/60 text-amber-300 hover:bg-amber-950/30'
          : 'border-zinc-800 text-zinc-300 hover:bg-zinc-800/60'}"
      >
        <span class="flex items-center gap-2">
          <span class="text-xs">{social ? '🎰' : '💵'}</span>
          Social casino
        </span>
        <span
          class="rounded-full px-2 py-0.5 text-[10px] font-semibold {social
            ? 'bg-amber-500/20 text-amber-300'
            : 'bg-zinc-800 text-zinc-400'}"
        >
          {social ? 'ON' : 'OFF'}
        </span>
      </button>
    </div>

    <button
      onclick={reloadAll}
      disabled={busy}
      class="mb-2 flex items-center justify-center gap-2 rounded-md bg-emerald-500 px-3 py-2 text-sm font-semibold text-zinc-950 transition hover:bg-emerald-400 disabled:cursor-not-allowed disabled:opacity-40"
    >
      <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h5M20 20v-5h-5M4 9a8 8 0 0114-3m2 8a8 8 0 01-14 3" />
      </svg>
      Apply &amp; reload all
    </button>

    <div class="mb-2 grid grid-cols-2 gap-1.5">
      <button
        onclick={muteAll}
        class="rounded-md border border-zinc-800 bg-zinc-900/60 px-2 py-1.5 text-[11px] font-medium text-zinc-300 transition hover:bg-zinc-800"
      >
        Mute all
      </button>
      <button
        onclick={unmuteAll}
        class="rounded-md border border-zinc-800 bg-zinc-900/60 px-2 py-1.5 text-[11px] font-medium text-zinc-300 transition hover:bg-zinc-800"
      >
        Unmute all
      </button>
    </div>

    <button
      onclick={() => (showManage = !showManage)}
      class="mb-2 flex items-center justify-between gap-2 rounded-md border border-zinc-800 bg-zinc-900/60 px-2.5 py-1.5 text-[11px] font-medium text-zinc-300 transition hover:bg-zinc-800"
    >
      <span class="flex items-center gap-2">
        <svg class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h7" />
        </svg>
        Manage resolutions ({allResolutions.filter((r) => r.enabled).length}/{allResolutions.length})
      </span>
      <svg
        class="h-3 w-3 text-zinc-500 transition {showManage ? 'rotate-180' : ''}"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
        viewBox="0 0 24 24"
      >
        <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
      </svg>
    </button>

    {#if showManage}
      <div class="mb-2 max-h-80 overflow-y-auto rounded-md border border-zinc-800 bg-zinc-950/40 p-2">
        <div class="mb-2 space-y-1">
          {#each allResolutions as r (r.id)}
            <div
              class="group flex items-center gap-2 rounded px-1.5 py-1 transition hover:bg-zinc-800/40"
            >
              <input
                id="res-{r.id}"
                name="res-{r.id}"
                type="checkbox"
                checked={r.enabled}
                onchange={(e) => toggleResolution(r.id, (e.currentTarget as HTMLInputElement).checked)}
                disabled={busy}
                class="accent-emerald-500"
              />
              <label for="res-{r.id}" class="flex-1 cursor-pointer text-xs">
                <span class="text-zinc-100">{r.label}</span>
                <span class="ml-1.5 font-mono text-[10px] text-zinc-500">{r.width}×{r.height}</span>
                {#if !r.builtin}
                  <span
                    class="ml-1 rounded bg-amber-500/15 px-1 py-0.5 text-[9px] font-semibold text-amber-300"
                    >custom</span
                  >
                {/if}
              </label>
              {#if !r.builtin}
                <button
                  onclick={() => deleteCustomResolution(r.id)}
                  disabled={busy}
                  title="Delete custom resolution"
                  class="rounded p-0.5 text-zinc-600 opacity-0 transition hover:bg-red-950/50 hover:text-red-400 group-hover:opacity-100"
                >
                  <svg class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              {/if}
            </div>
          {/each}
        </div>

        <div class="border-t border-zinc-800 pt-2">
          <div class="mb-1 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
            Add custom
          </div>
          <input
            type="text"
            bind:value={newCustomLabel}
            placeholder="Label (e.g. iPad)"
            class="mb-1.5 w-full rounded border border-zinc-800 bg-zinc-950/60 px-2 py-1 text-xs focus:border-emerald-500/40 focus:outline-none"
          />
          <div class="mb-1.5 grid grid-cols-2 gap-1.5">
            <input
              type="number"
              bind:value={newCustomWidth}
              min="1"
              max="4096"
              placeholder="Width"
              class="rounded border border-zinc-800 bg-zinc-950/60 px-2 py-1 font-mono text-xs focus:border-emerald-500/40 focus:outline-none"
            />
            <input
              type="number"
              bind:value={newCustomHeight}
              min="1"
              max="4096"
              placeholder="Height"
              class="rounded border border-zinc-800 bg-zinc-950/60 px-2 py-1 font-mono text-xs focus:border-emerald-500/40 focus:outline-none"
            />
          </div>
          <button
            onclick={addCustomResolution}
            disabled={busy || !newCustomLabel.trim()}
            class="w-full rounded bg-emerald-500 px-2 py-1 text-xs font-semibold text-zinc-950 transition hover:bg-emerald-400 disabled:opacity-40"
          >
            + Add
          </button>
        </div>
      </div>
    {/if}

    <div class="mt-auto space-y-2">
      {#if info}
        <div
          class="rounded-md border border-emerald-900/40 bg-emerald-950/30 px-2.5 py-1.5 text-[11px] text-emerald-300"
        >
          {info}
        </div>
      {/if}
      {#if error}
        <div
          class="rounded-md border border-red-900/40 bg-red-950/30 px-2.5 py-1.5 text-[11px] text-red-300"
        >
          {error}
        </div>
      {/if}
    </div>
  </aside>

  <!-- Frames area -->
  <main class="flex-1 overflow-auto p-6">
    <div class="flex flex-wrap gap-6">
      {#each frames as frame (frame.res.id)}
        <div class="flex flex-col">
          <div class="mb-1.5 flex items-center justify-between gap-3">
            <div class="text-xs">
              <span class="font-semibold text-zinc-100">{frame.res.label}</span>
              <span class="ml-1.5 font-mono text-zinc-500">
                {frame.res.width}×{frame.res.height}
              </span>
            </div>
            <div class="flex items-center gap-1.5">
              <button
                onclick={() => toggleMute(frame)}
                title={frame.muted ? 'Unmute (allow audio)' : 'Mute (block clicks → suspend audio)'}
                class="rounded border border-zinc-800 bg-zinc-900/60 p-1 transition hover:bg-zinc-800 {frame.muted
                  ? 'text-amber-400'
                  : 'text-emerald-400'}"
              >
                {#if frame.muted}
                  <svg class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
                    <path stroke-linecap="round" stroke-linejoin="round" d="M17 14l4-4m0 4l-4-4" />
                  </svg>
                {:else}
                  <svg class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
                    <path stroke-linecap="round" stroke-linejoin="round" d="M15.536 8.464a5 5 0 010 7.072M19.07 4.93a10 10 0 010 14.14" />
                  </svg>
                {/if}
              </button>
              <button
                onclick={() => reloadOne(frame)}
                disabled={busy}
                title="Reload this frame"
                class="rounded border border-zinc-800 bg-zinc-900/60 p-1 text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-100 disabled:opacity-40"
              >
                <svg class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h5M20 20v-5h-5M4 9a8 8 0 0114-3m2 8a8 8 0 01-14 3" />
                </svg>
              </button>
              <button
                onclick={() => openInBrowser(frame)}
                disabled={busy || !frame.src}
                title="Open in new tab"
                class="rounded border border-zinc-800 bg-zinc-900/60 p-1 text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-100 disabled:opacity-40"
              >
                <svg class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M14 3h7v7m0 -7L10 14M5 5h4v4M5 19h14v-4" />
                </svg>
              </button>
            </div>
          </div>
          <div
            class="relative overflow-hidden rounded-lg border border-zinc-800/60 bg-black shadow-xl"
            style="width: {frame.res.width}px; height: {frame.res.height}px;"
          >
            {#if frame.src}
              <iframe
                src={frame.src}
                title={frame.res.label}
                class="h-full w-full bg-black"
                allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; fullscreen; cross-origin-isolated"
                allowfullscreen
              ></iframe>
            {:else}
              <div class="flex h-full w-full items-center justify-center text-xs text-zinc-600">
                Loading…
              </div>
            {/if}

            {#if frame.muted && frame.src}
              <!-- Click-blocking transparent overlay. Browsers gate audio behind
                   user gestures; without clicks the iframe's AudioContext stays
                   suspended → effectively muted. -->
              <button
                onclick={() => toggleMute(frame)}
                class="group absolute inset-0 z-10 flex cursor-pointer items-center justify-center bg-black/0 transition hover:bg-black/30"
                title="Click to unmute (enable audio + interactions)"
                aria-label="Unmute"
              >
                <span class="rounded-full bg-zinc-900/80 p-2 text-amber-400 opacity-0 ring-1 ring-amber-500/30 transition group-hover:opacity-100">
                  <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
                    <path stroke-linecap="round" stroke-linejoin="round" d="M17 14l4-4m0 4l-4-4" />
                  </svg>
                </span>
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </main>
</div>
