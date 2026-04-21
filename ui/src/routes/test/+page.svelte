<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { page } from '$app/state';
  import Picker from '$lib/Picker.svelte';
  import {
    LANGUAGES,
    CURRENCIES,
    API_MULTIPLIER,
    settingsHttp,
    forcedEventHttp,
    savedRoundsHttp,
    betStatsHttp,
    gameModesHttp,
    replayUrl,
    type ResolutionPreset,
    type EventEntry,
    type SavedRound,
    type ModeBetStats
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
    history: EventEntry[];
    showHistory: boolean;
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
  let showReplay = $state(false);
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
        existing.res = res;
        return existing;
      }
      return {
        res,
        sessionId: crypto.randomUUID(),
        src: null,
        muted: true,
        history: [],
        showHistory: false
      };
    });
  }

  // ---- Modes (from the game's index.json, loaded on mount) ----

  // Falls back to ['base'] while loading so the dropdowns are never empty. Once
  // the modes endpoint responds with the real list we overwrite it and snap
  // forcedMode/replayMode onto a valid entry.
  let availableModes = $state<string[]>(['base']);

  // ---- Force event + last event + replay ----

  let forcedMode = $state<string>('base');
  let forcedEventId = $state<number | null>(null);
  let forcedEventBanner = $state<{ mode: string; eventId: number } | null>(null);

  let replayMode = $state<string>('base');
  let replayEventId = $state<number | null>(null);

  // ---- Saved rounds ----

  let savedRounds = $state<SavedRound[]>([]);
  let showSavedRounds = $state(true);
  let savingRound = $state(false);
  let saveDescription = $state('');
  let showSaveInput = $state(false);

  // ---- Notable rounds (computed from books per mode) ----

  let notableRounds = $state<ModeBetStats[]>([]);
  let notableLoading = $state(false);
  let notableLoaded = $state(false);
  let showNotable = $state(false);

  async function reloadSavedRounds() {
    if (!gameSlug) return;
    try {
      savedRounds = await savedRoundsHttp.list(gameSlug);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function loadNotableRounds() {
    if (!gameSlug || notableLoading) return;
    notableLoading = true;
    try {
      notableRounds = await betStatsHttp.get(gameSlug);
      notableLoaded = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      notableLoading = false;
    }
  }

  async function toggleNotablePanel() {
    showNotable = !showNotable;
    if (showNotable && !notableLoaded) await loadNotableRounds();
  }

  async function applyForcedFromNotable(mode: string, eventId: number) {
    forcedMode = mode;
    forcedEventId = eventId;
    busy = true;
    try {
      const resp = await forcedEventHttp.set(mode, eventId);
      forcedEventBanner = resp.forced;
      info = `Forced: ${mode} #${eventId}.`;
      setTimeout(() => (info = null), 2500);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  /** Bookmark a notable bet with the auto-set description ("min" / "average win"
   *  / "max win"). No popup — these descriptions are canonical. */
  async function bookmarkNotable(mode: string, eventId: number, kind: 'min' | 'avg' | 'max') {
    if (!gameSlug) return;
    if (isBookmarked(mode, eventId)) return;
    const description = kind === 'min' ? 'min' : kind === 'avg' ? 'average win' : 'max win';
    try {
      await savedRoundsHttp.create(gameSlug, mode, eventId, description);
      await reloadSavedRounds();
      info = `Bookmarked ${mode} #${eventId} (${description}).`;
      setTimeout(() => (info = null), 2000);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function saveCurrentRound() {
    if (forcedEventId === null || forcedEventId <= 0) {
      error = 'Enter a valid event id before saving.';
      return;
    }
    if (!gameSlug) return;
    savingRound = true;
    try {
      await savedRoundsHttp.create(
        gameSlug,
        forcedMode,
        forcedEventId,
        saveDescription.trim()
      );
      saveDescription = '';
      showSaveInput = false;
      await reloadSavedRounds();
      info = `Saved ${forcedMode} #${forcedEventId}.`;
      setTimeout(() => (info = null), 2000);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      savingRound = false;
    }
  }

  async function applySavedRound(r: SavedRound) {
    forcedMode = r.mode;
    forcedEventId = r.eventId;
    busy = true;
    try {
      const resp = await forcedEventHttp.set(r.mode, r.eventId);
      forcedEventBanner = resp.forced;
      info = `Forced: ${r.mode} #${r.eventId}${r.description ? ` — ${r.description}` : ''}.`;
      setTimeout(() => (info = null), 3000);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function deleteSavedRound(r: SavedRound) {
    if (!confirm(`Delete saved round ${r.mode} #${r.eventId}?`)) return;
    try {
      await savedRoundsHttp.remove(r.id);
      await reloadSavedRounds();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function isBookmarked(mode: string, eventId: number): boolean {
    return savedRounds.some((r) => r.mode === mode && r.eventId === eventId);
  }

  // Modal state for "Bookmark from history" — opens when the user clicks ★ on
  // a history row so they can attach a description before saving.
  let bookmarkModal = $state<{
    mode: string;
    eventId: number;
    description: string;
    saving: boolean;
  } | null>(null);

  function openBookmarkModal(entry: EventEntry) {
    if (isBookmarked(entry.mode, entry.eventId)) return;
    bookmarkModal = {
      mode: entry.mode,
      eventId: entry.eventId,
      description: '',
      saving: false
    };
  }

  function closeBookmarkModal() {
    bookmarkModal = null;
  }

  // Focus the description input when the modal opens (replaces `autofocus`,
  // which svelte-check flags as an a11y antipattern).
  let bookmarkInputEl = $state<HTMLInputElement | null>(null);
  $effect(() => {
    if (bookmarkModal && bookmarkInputEl) {
      bookmarkInputEl.focus();
    }
  });

  async function confirmBookmark() {
    if (!bookmarkModal || !gameSlug) return;
    bookmarkModal.saving = true;
    try {
      await savedRoundsHttp.create(
        gameSlug,
        bookmarkModal.mode,
        bookmarkModal.eventId,
        bookmarkModal.description.trim()
      );
      await reloadSavedRounds();
      info = `Bookmarked ${bookmarkModal.mode} #${bookmarkModal.eventId}.`;
      setTimeout(() => (info = null), 2000);
      bookmarkModal = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      if (bookmarkModal) bookmarkModal.saving = false;
    }
  }

  async function applyForcedEvent() {
    if (forcedEventId === null || forcedEventId <= 0) {
      error = 'Enter a valid event id.';
      return;
    }
    busy = true;
    try {
      const r = await forcedEventHttp.set(forcedMode, forcedEventId);
      forcedEventBanner = r.forced;
      info = `Forced: mode=${forcedMode}, eventId=${forcedEventId}. Every /play in this mode now returns this event.`;
      setTimeout(() => (info = null), 3000);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function clearForcedEvent() {
    busy = true;
    try {
      await forcedEventHttp.clear();
      forcedEventBanner = null;
      info = 'Forced event cleared — back to RNG.';
      setTimeout(() => (info = null), 2500);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function launchReplay(frame: FrameState) {
    if (replayEventId === null || replayEventId <= 0) {
      error = 'Enter a valid event id to replay.';
      return;
    }
    const url = replayUrl(gameUrl, gameSlug, lgsHostPort, {
      mode: replayMode,
      eventId: replayEventId,
      currency,
      amount: Math.round(balance * API_MULTIPLIER),
      lang: language,
      device,
      social
    });
    frame.src = url;
    info = `Replay launched on ${frame.res.label}: ${replayMode} #${replayEventId}.`;
    setTimeout(() => (info = null), 2500);
  }

  // One persistent SSE connection per frame (keyed by sessionId). The server
  // emits a `snapshot` event on connect (full current history) then pushes
  // each new event as it happens. No polling, no wasted requests.
  const eventSources = new Map<string, EventSource>();
  const HISTORY_CAP = 100;

  $effect(() => {
    const activeSids = new Set(frames.map((f) => f.sessionId));

    for (const f of frames) {
      if (eventSources.has(f.sessionId)) continue;
      const frame = f;
      const es = new EventSource(
        `/api/devtool/sessions/${encodeURIComponent(frame.sessionId)}/stream`
      );
      es.addEventListener('snapshot', (ev) => {
        try {
          frame.history = JSON.parse((ev as MessageEvent).data) as EventEntry[];
        } catch {
          // ignore malformed snapshot
        }
      });
      es.addEventListener('event', (ev) => {
        try {
          const entry = JSON.parse((ev as MessageEvent).data) as EventEntry;
          // De-dup: server sends the snapshot then subscribes, so an entry
          // pushed in that window can arrive via both paths. Skip if head
          // already matches.
          const head = frame.history[0];
          if (head && head.at === entry.at && head.eventId === entry.eventId) return;
          const next = [entry, ...frame.history];
          if (next.length > HISTORY_CAP) next.length = HISTORY_CAP;
          frame.history = next;
        } catch {
          // ignore malformed event
        }
      });
      eventSources.set(frame.sessionId, es);
    }

    for (const [sid, es] of eventSources) {
      if (!activeSids.has(sid)) {
        es.close();
        eventSources.delete(sid);
      }
    }
  });

  onDestroy(() => {
    for (const es of eventSources.values()) es.close();
    eventSources.clear();
  });

  function formatRelative(ms: number): string {
    const diff = Date.now() - ms;
    if (diff < 1000) return 'just now';
    if (diff < 60_000) return `${Math.floor(diff / 1000)}s ago`;
    if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
    return `${Math.floor(diff / 3_600_000)}h ago`;
  }

  function formatAmount(microUnits: number): string {
    return (microUnits / API_MULTIPLIER).toFixed(2);
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
      const f = await forcedEventHttp.get();
      forcedEventBanner = f.forced;
      await reloadSavedRounds();
      await loadGameModes();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      return;
    }
    await reloadAll();
  });

  /** Read the mode list from the game's index.json (via LGS) and align the
   *  current forcedMode / replayMode with what the game actually exposes.
   *  Failure keeps the fallback ['base']; dropdowns still render. */
  async function loadGameModes() {
    if (!gameSlug) return;
    try {
      const modes = await gameModesHttp.get(gameSlug);
      if (modes.length === 0) return;
      availableModes = modes;
      if (!modes.includes(forcedMode)) forcedMode = modes[0];
      if (!modes.includes(replayMode)) replayMode = modes[0];
    } catch (e) {
      // Non-fatal: the dropdowns stay on the ['base'] fallback.
      console.warn('failed to load game modes:', e);
    }
  }

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
    const res = await fetch(`${lgsBase}/api/devtool/sessions/prepare`, {
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

  /** Cross-origin iframes don't let the parent pause their AudioContext. The
   *  only way to actually silence an iframe that's already playing is to
   *  recycle it: clear src so the element is destroyed, then set it back after
   *  a tick so Svelte creates a fresh iframe. The sessionId is preserved so
   *  balance/history survive. */
  function remountIframe(frame: FrameState) {
    if (!frame.src) return;
    const src = frame.src;
    frame.src = null;
    setTimeout(() => {
      frame.src = src;
    }, 30);
  }

  function toggleMute(frame: FrameState) {
    const wasMuted = frame.muted;
    frame.muted = !frame.muted;
    // When re-muting a frame whose audio is already running, we must remount.
    if (!wasMuted && frame.muted) remountIframe(frame);
  }

  function muteAll() {
    for (const f of frames) {
      const wasMuted = f.muted;
      f.muted = true;
      if (!wasMuted) remountIframe(f);
    }
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
  <aside class="flex h-screen w-96 flex-shrink-0 flex-col border-r border-zinc-800/60 bg-zinc-900/40">
    <!-- Header -->
    <div class="flex-shrink-0 border-b border-zinc-800/60 px-5 py-4">
      <div class="text-[10px] font-medium uppercase tracking-wider text-zinc-500">Game</div>
      <div class="mt-0.5 truncate text-sm font-semibold text-zinc-100">{gameSlug || '—'}</div>
    </div>

    <!-- Scrollable sections -->
    <div class="flex-1 overflow-y-auto px-5 py-4">
      <!-- ========== SESSION ========== -->
      <section class="mb-5">
        <div class="mb-2 text-[10px] font-semibold uppercase tracking-wider text-zinc-500">
          Session
        </div>

        <div class="space-y-3">
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

          <button
            onclick={reloadAll}
            disabled={busy}
            class="flex w-full items-center justify-center gap-2 rounded-md bg-emerald-500 px-3 py-2 text-sm font-semibold text-zinc-950 transition hover:bg-emerald-400 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h5M20 20v-5h-5M4 9a8 8 0 0114-3m2 8a8 8 0 01-14 3" />
            </svg>
            Apply &amp; reload all
          </button>

          <div class="grid grid-cols-2 gap-1.5">
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
        </div>
      </section>

      <!-- ========== EVENTS ========== -->
      <section class="mb-5">
        <div class="mb-2 text-[10px] font-semibold uppercase tracking-wider text-zinc-500">
          Events
        </div>

        <!-- Force event -->
        <div class="mb-2 rounded-md border border-zinc-800 bg-zinc-950/40 p-2">
          <div class="mb-1 flex items-center justify-between">
            <span class="text-[10px] font-medium uppercase tracking-wider text-zinc-500">
              Force next event
            </span>
            {#if forcedEventBanner}
              <button
                onclick={clearForcedEvent}
                disabled={busy}
                class="text-[10px] text-amber-400 hover:text-amber-300"
              >
                Clear
              </button>
            {/if}
          </div>
          <div class="flex gap-1.5">
            <select
              bind:value={forcedMode}
              class="rounded border border-zinc-800 bg-zinc-950/60 px-1.5 py-1 font-mono text-[11px] focus:border-emerald-500/40 focus:outline-none"
            >
              {#each availableModes as m (m)}
                <option value={m}>{m}</option>
              {/each}
            </select>
            <input
              type="number"
              bind:value={forcedEventId}
              min="1"
              placeholder="eventId"
              class="flex-1 rounded border border-zinc-800 bg-zinc-950/60 px-2 py-1 font-mono text-[11px] focus:border-emerald-500/40 focus:outline-none"
            />
            <button
              onclick={applyForcedEvent}
              disabled={busy}
              class="rounded bg-amber-500 px-2 py-1 text-[11px] font-semibold text-zinc-950 transition hover:bg-amber-400 disabled:opacity-40"
            >
              Force
            </button>
            <button
              onclick={() => (showSaveInput = !showSaveInput)}
              disabled={forcedEventId === null || forcedEventId <= 0}
              title="Save this round for later"
              class="rounded border border-zinc-800 bg-zinc-900/60 px-2 py-1 text-[11px] font-semibold text-zinc-300 transition hover:bg-zinc-800 disabled:opacity-40"
            >
              ★
            </button>
          </div>
          {#if showSaveInput}
            <div class="mt-1.5 flex gap-1.5">
              <input
                type="text"
                bind:value={saveDescription}
                placeholder="Description (optional)"
                maxlength="120"
                onkeydown={(e) => e.key === 'Enter' && saveCurrentRound()}
                class="flex-1 rounded border border-zinc-800 bg-zinc-950/60 px-2 py-1 text-[11px] focus:border-emerald-500/40 focus:outline-none"
              />
              <button
                onclick={saveCurrentRound}
                disabled={savingRound}
                class="rounded bg-emerald-500 px-2 py-1 text-[11px] font-semibold text-zinc-950 transition hover:bg-emerald-400 disabled:opacity-40"
              >
                Save
              </button>
            </div>
          {/if}
          {#if forcedEventBanner}
            <div class="mt-1.5 rounded bg-amber-500/10 px-2 py-1 text-[10px] text-amber-300">
              Active: <span class="font-mono">{forcedEventBanner.mode} #{forcedEventBanner.eventId}</span>
            </div>
          {/if}
        </div>

        <!-- Saved rounds -->
        <div class="mb-2 rounded-md border border-zinc-800 bg-zinc-950/40">
          <button
            onclick={() => (showSavedRounds = !showSavedRounds)}
            class="flex w-full items-center justify-between gap-2 px-2 py-1.5 text-[10px] font-medium uppercase tracking-wider text-zinc-400 transition hover:text-zinc-200"
          >
            <span>Saved rounds ({savedRounds.length})</span>
            <svg
              class="h-3 w-3 transition {showSavedRounds ? 'rotate-180' : ''}"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              viewBox="0 0 24 24"
            >
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
            </svg>
          </button>
          {#if showSavedRounds}
            <div class="border-t border-zinc-800/60 p-2">
              {#if savedRounds.length === 0}
                <div class="text-[10px] text-zinc-600">
                  No saved rounds yet. Enter a mode + event id above and click ★ to bookmark.
                </div>
              {:else}
                <div class="max-h-56 space-y-1 overflow-y-auto">
                  {#each savedRounds as r (r.id)}
                    <div class="group flex items-center gap-1.5 rounded px-1.5 py-1 transition hover:bg-zinc-800/40">
                      <button
                        onclick={() => applySavedRound(r)}
                        disabled={busy}
                        title="Force this round"
                        class="flex min-w-0 flex-1 flex-col items-start text-left"
                      >
                        <span class="flex w-full items-baseline gap-1.5">
                          <span class="font-mono text-[11px] text-sky-300">{r.mode}</span>
                          <span class="font-mono text-[11px] font-semibold text-zinc-100">#{r.eventId}</span>
                        </span>
                        {#if r.description}
                          <span class="w-full truncate text-[10px] text-zinc-400">{r.description}</span>
                        {/if}
                      </button>
                      <button
                        onclick={() => deleteSavedRound(r)}
                        title="Delete"
                        class="rounded p-0.5 text-zinc-600 opacity-0 transition hover:bg-red-950/50 hover:text-red-400 group-hover:opacity-100"
                      >
                        <svg class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                      </button>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        </div>

        <!-- Notable rounds (auto-detected from books) -->
        <div class="mb-2 rounded-md border border-zinc-800 bg-zinc-950/40">
          <button
            onclick={toggleNotablePanel}
            class="flex w-full items-center justify-between gap-2 px-2 py-1.5 text-[10px] font-medium uppercase tracking-wider text-zinc-400 transition hover:text-zinc-200"
          >
            <span>
              Notable rounds
              {#if notableLoaded}
                <span class="text-zinc-600">({notableRounds.length})</span>
              {/if}
            </span>
            <svg
              class="h-3 w-3 transition {showNotable ? 'rotate-180' : ''}"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              viewBox="0 0 24 24"
            >
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
            </svg>
          </button>
          {#if showNotable}
            <div class="border-t border-zinc-800/60 p-2">
              {#if notableLoading}
                <div class="text-[10px] text-zinc-500">Loading…</div>
              {:else if notableRounds.length === 0}
                <div class="text-[10px] text-zinc-600">
                  No modes detected. Make sure the math folder has weights.
                </div>
              {:else}
                <div class="text-[9px] uppercase tracking-wider text-zinc-600 mb-1">
                  Auto-picked from each mode's lookup table. Click ★ to bookmark.
                </div>
                <div class="max-h-72 space-y-2 overflow-y-auto">
                  {#each notableRounds as m (m.mode)}
                    <div class="rounded border border-zinc-800/60 bg-zinc-950/40 p-1.5">
                      <div class="mb-1 font-mono text-[11px] font-semibold text-sky-300">
                        {m.mode}
                      </div>
                      <div class="space-y-0.5">
                        {#each [
                          { kind: 'min' as const, label: 'min', bet: m.stats.min, color: 'text-zinc-400' },
                          { kind: 'avg' as const, label: 'avg', bet: m.stats.avg, color: 'text-amber-300' },
                          { kind: 'max' as const, label: 'max', bet: m.stats.max, color: 'text-emerald-300' }
                        ] as row (row.kind)}
                          {@const bk = isBookmarked(m.mode, row.bet.eventId)}
                          <div class="flex items-center gap-2 rounded px-1.5 py-0.5 hover:bg-zinc-800/40">
                            <span class="w-7 text-[9px] uppercase tracking-wider text-zinc-500">
                              {row.label}
                            </span>
                            <span class="font-mono text-[11px] {row.color}">
                              #{row.bet.eventId}
                            </span>
                            <span class="ml-auto font-mono text-[10px] text-zinc-500">
                              ×{(row.bet.payoutMultiplier / 100).toFixed(2)}
                            </span>
                            <button
                              onclick={() => applyForcedFromNotable(m.mode, row.bet.eventId)}
                              disabled={busy}
                              title="Force this round"
                              class="rounded border border-zinc-800 bg-zinc-900/60 px-1.5 py-0.5 text-[9px] font-semibold text-zinc-300 transition hover:bg-amber-500 hover:text-zinc-950 disabled:opacity-40"
                            >
                              Force
                            </button>
                            <button
                              onclick={() => bookmarkNotable(m.mode, row.bet.eventId, row.kind)}
                              disabled={bk}
                              title={bk ? 'Already bookmarked' : `Bookmark as "${row.kind === 'min' ? 'min' : row.kind === 'avg' ? 'average win' : 'max win'}"`}
                              class="leading-none transition {bk
                                ? 'cursor-default text-amber-400'
                                : 'text-zinc-600 hover:text-amber-400'}"
                            >
                              {bk ? '★' : '☆'}
                            </button>
                          </div>
                        {/each}
                      </div>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        </div>

        <!-- Replay -->
        <div class="rounded-md border border-zinc-800 bg-zinc-950/40">
          <button
            onclick={() => (showReplay = !showReplay)}
            class="flex w-full items-center justify-between gap-2 px-2 py-1.5 text-[10px] font-medium uppercase tracking-wider text-zinc-400 transition hover:text-zinc-200"
          >
            <span>Replay event</span>
            <svg
              class="h-3 w-3 transition {showReplay ? 'rotate-180' : ''}"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              viewBox="0 0 24 24"
            >
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
            </svg>
          </button>
          {#if showReplay}
            <div class="border-t border-zinc-800/60 p-2">
              <div class="flex gap-1.5">
                <select
                  bind:value={replayMode}
                  class="rounded border border-zinc-800 bg-zinc-950/60 px-1.5 py-1 font-mono text-[11px] focus:border-emerald-500/40 focus:outline-none"
                >
                  {#each availableModes as m (m)}
                    <option value={m}>{m}</option>
                  {/each}
                </select>
                <input
                  type="number"
                  bind:value={replayEventId}
                  min="1"
                  placeholder="eventId"
                  class="flex-1 rounded border border-zinc-800 bg-zinc-950/60 px-2 py-1 font-mono text-[11px] focus:border-emerald-500/40 focus:outline-none"
                />
                <button
                  onclick={() => frames[0] && launchReplay(frames[0])}
                  disabled={busy || frames.length === 0}
                  title="Load replay in the first frame"
                  class="rounded bg-sky-500 px-2 py-1 text-[11px] font-semibold text-zinc-950 transition hover:bg-sky-400 disabled:opacity-40"
                >
                  Load
                </button>
              </div>
              <div class="mt-1 text-[10px] text-zinc-600">
                Loads into the top-left frame. No session, no RNG — just the event outcome.
              </div>
            </div>
          {/if}
        </div>
      </section>

      <!-- ========== LAYOUT ========== -->
      <section>
        <div class="mb-2 text-[10px] font-semibold uppercase tracking-wider text-zinc-500">
          Layout
        </div>

        <div class="rounded-md border border-zinc-800 bg-zinc-950/40">
          <button
            onclick={() => (showManage = !showManage)}
            class="flex w-full items-center justify-between gap-2 px-2.5 py-1.5 text-[11px] font-medium text-zinc-300 transition hover:text-zinc-100"
          >
            <span class="flex items-center gap-2">
              <svg class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h7" />
              </svg>
              Resolutions ({allResolutions.filter((r) => r.enabled).length}/{allResolutions.length})
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
            <div class="border-t border-zinc-800/60 p-2">
              <div class="mb-2 max-h-64 space-y-1 overflow-y-auto">
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
        </div>
      </section>
    </div>

    <!-- Footer: messages -->
    {#if info || error}
      <div class="flex-shrink-0 space-y-2 border-t border-zinc-800/60 px-5 py-3">
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
    {/if}
  </aside>

  <!-- Frames area -->
  <main class="flex-1 overflow-auto p-6">
    <div class="flex flex-wrap gap-6">
      {#each frames as frame (frame.res.id)}
        <div class="flex flex-col">
          <div class="mb-1.5 flex items-center justify-between gap-3">
            <div class="flex items-center gap-2 text-xs">
              <span class="font-semibold text-zinc-100">{frame.res.label}</span>
              <span class="font-mono text-zinc-500">
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
          <!-- Big last-event strip (above the iframe, full-width of frame) -->
          <div
            class="mb-1 flex items-center gap-3 rounded-md border border-zinc-800/60 bg-zinc-900/60 px-3 py-1.5"
            style="width: {frame.res.width}px;"
          >
            {#if frame.history[0]}
              {@const last = frame.history[0]}
              {@const lm = last.payoutMultiplier / 100}
              {@const hit = lm > 0}
              <div class="flex items-baseline gap-2.5 text-sm">
                <span class="text-[10px] uppercase tracking-wider text-zinc-500">Last</span>
                <span class="font-mono font-semibold text-sky-300">#{last.eventId}</span>
                <span class="font-mono text-base font-bold {hit ? 'text-emerald-400' : 'text-zinc-500'}">
                  ×{lm.toFixed(2)}
                </span>
                <span class="font-mono text-xs text-zinc-400">
                  bet {formatAmount(last.betAmount)} · win {formatAmount(last.payout)}
                </span>
              </div>
            {:else}
              <span class="text-xs text-zinc-600">Waiting for first spin…</span>
            {/if}
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

          <!-- History toggle (directly below the iframe so the panel it
               opens lives in the same vertical zone as its trigger) -->
          <button
            onclick={() => (frame.showHistory = !frame.showHistory)}
            disabled={frame.history.length === 0}
            title="Toggle event history"
            style="width: {frame.res.width}px;"
            class="mt-1 flex items-center justify-between gap-2 rounded-md border border-zinc-800/60 bg-zinc-900/60 px-3 py-1 text-[10px] font-medium uppercase tracking-wider text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-100 disabled:opacity-40"
          >
            <span>Bet history ({frame.history.length})</span>
            <svg class="h-3 w-3 transition {frame.showHistory ? 'rotate-180' : ''}" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
            </svg>
          </button>

          {#if frame.showHistory && frame.history.length > 0}
            <div
              class="mt-1 overflow-hidden rounded-md border border-zinc-800/60 bg-zinc-950/40"
              style="width: {frame.res.width}px;"
            >
              <div class="grid grid-cols-[auto_auto_auto_1fr_auto_auto_auto] items-center gap-x-3 border-b border-zinc-800 bg-zinc-900/60 px-3 py-1 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
                <span></span>
                <span>#</span>
                <span>Event</span>
                <span>Mode</span>
                <span>Bet</span>
                <span>Mult</span>
                <span>Win</span>
              </div>
              <div class="max-h-64 overflow-y-auto font-mono text-xs">
                {#each frame.history as entry, i (entry.at + '-' + entry.eventId)}
                  {@const hit = entry.payout > 0}
                  {@const bookmarked = isBookmarked(entry.mode, entry.eventId)}
                  <div
                    class="grid grid-cols-[auto_auto_auto_1fr_auto_auto_auto] items-center gap-x-3 border-b border-zinc-900/40 px-3 py-1 transition hover:bg-zinc-800/30 {entry.forced
                      ? 'bg-amber-500/5'
                      : ''}"
                  >
                    <button
                      onclick={() => openBookmarkModal(entry)}
                      disabled={bookmarked}
                      title={bookmarked ? 'Already bookmarked' : 'Bookmark this round'}
                      class="leading-none transition {bookmarked
                        ? 'cursor-default text-amber-400'
                        : 'text-zinc-600 hover:text-amber-400'}"
                    >
                      {bookmarked ? '★' : '☆'}
                    </button>
                    <span class="text-zinc-600">{i + 1}</span>
                    <span class="font-semibold text-sky-300">#{entry.eventId}</span>
                    <span class="truncate text-zinc-400">
                      {entry.mode}
                      {#if entry.forced}
                        <span class="ml-1 rounded bg-amber-500/20 px-1 py-0.5 text-[9px] font-semibold text-amber-300">
                          FORCED
                        </span>
                      {/if}
                    </span>
                    <span class="text-zinc-400">{formatAmount(entry.betAmount)}</span>
                    <span class="{hit ? 'text-emerald-400' : 'text-zinc-600'}">
                      ×{(entry.payoutMultiplier / 100).toFixed(2)}
                    </span>
                    <span class="{hit ? 'text-emerald-400' : 'text-zinc-600'}">
                      {formatAmount(entry.payout)}
                    </span>
                  </div>
                {/each}
              </div>
              <div class="border-t border-zinc-800 bg-zinc-900/40 px-3 py-1 text-[10px] text-zinc-500">
                Last 100 spins, newest first. Resets when the frame is reloaded.
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </main>

  {#if bookmarkModal}
    <!-- Backdrop + centered modal. Click backdrop or Esc to cancel. -->
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="bookmark-modal-title"
      tabindex="-1"
      onkeydown={(e) => {
        if (e.key === 'Escape') closeBookmarkModal();
      }}
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
    >
      <button
        type="button"
        aria-label="Close"
        onclick={closeBookmarkModal}
        class="absolute inset-0 cursor-default"
      ></button>
      <div
        class="relative w-[420px] max-w-[90vw] rounded-lg border border-zinc-800 bg-zinc-900 p-5 shadow-2xl"
      >
        <div class="mb-3 flex items-center justify-between">
          <h2 id="bookmark-modal-title" class="text-sm font-semibold text-zinc-100">
            Bookmark this round
          </h2>
          <button
            onclick={closeBookmarkModal}
            class="rounded p-1 text-zinc-500 transition hover:bg-zinc-800 hover:text-zinc-200"
            aria-label="Cancel"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div class="mb-3 flex items-baseline gap-2 rounded-md border border-zinc-800 bg-zinc-950/60 px-3 py-2">
          <span class="text-[10px] uppercase tracking-wider text-zinc-500">Round</span>
          <span class="font-mono text-sm text-sky-300">{bookmarkModal.mode}</span>
          <span class="font-mono text-sm font-semibold text-zinc-100">
            #{bookmarkModal.eventId}
          </span>
        </div>
        <label
          for="bookmark-description"
          class="mb-1 block text-[10px] font-medium uppercase tracking-wider text-zinc-500"
        >
          Description (optional)
        </label>
        <input
          id="bookmark-description"
          type="text"
          bind:this={bookmarkInputEl}
          bind:value={bookmarkModal.description}
          placeholder="e.g. Big bonus trigger, near miss, …"
          maxlength="120"
          onkeydown={(e) => {
            if (e.key === 'Enter') confirmBookmark();
          }}
          class="mb-4 w-full rounded-md border border-zinc-800 bg-zinc-950/60 px-2.5 py-1.5 text-sm focus:border-emerald-500/50 focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
        />
        <div class="flex justify-end gap-2">
          <button
            onclick={closeBookmarkModal}
            class="rounded-md border border-zinc-800 bg-zinc-900 px-3 py-1.5 text-xs font-medium text-zinc-300 transition hover:bg-zinc-800"
          >
            Cancel
          </button>
          <button
            onclick={confirmBookmark}
            disabled={bookmarkModal.saving}
            class="flex items-center gap-1.5 rounded-md bg-emerald-500 px-3 py-1.5 text-xs font-semibold text-zinc-950 transition hover:bg-emerald-400 disabled:opacity-40"
          >
            <span>★</span>
            {bookmarkModal.saving ? 'Saving…' : 'Bookmark'}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
