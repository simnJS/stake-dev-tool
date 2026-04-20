<script lang="ts">
  import { onMount } from 'svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import {
    browser,
    ca,
    checkForUpdates,
    downloadAndInstallUpdate,
    lgs,
    pickFolder,
    profiles as profilesApi,
    settings as settingsApi,
    type CaStatus,
    type InspectedGame,
    type LgsStatus,
    type Profile,
    type ResolutionPreset,
    type UpdateInfo
  } from '$lib/api';

  const DEFAULT_PORT = 3001;

  let status = $state<LgsStatus>({ running: false, bound_addr: null, math_dir: null });
  let caState = $state<CaStatus>({ installed: false, caPath: '' });

  let game = $state<InspectedGame | null>(null);
  let gameUrl = $state('http://localhost:5174');

  let savedProfiles = $state<Profile[]>([]);
  let activeProfileId = $state<string | null>(null);
  let showSaveDialog = $state(false);
  let saveName = $state('');

  let resolutions = $state<ResolutionPreset[]>([]);
  let showResolutions = $state(false);

  let updateInfo = $state<UpdateInfo | null>(null);
  let checkingUpdate = $state(false);
  let installingUpdate = $state(false);
  let updateProgress = $state<{ downloaded: number; total?: number } | null>(null);

  let error = $state<string | null>(null);
  let info = $state<string | null>(null);
  let busy = $state(false);

  onMount(async () => {
    try {
      status = await lgs.status();
      caState = await ca.status();
      savedProfiles = await profilesApi.list();
      const s = await settingsApi.get();
      resolutions = s.resolutions;
    } catch (e) {
      console.error(e);
    }
    // Silent update check on startup — don't block the UI.
    checkUpdate(true).catch(() => {});
  });

  async function checkUpdate(silent = false) {
    checkingUpdate = true;
    if (!silent) {
      error = null;
      info = null;
    }
    try {
      updateInfo = await checkForUpdates();
      if (!silent) {
        info = updateInfo.available
          ? `Update available: v${updateInfo.version}`
          : `You're up to date (v${updateInfo.currentVersion}).`;
      }
    } catch (e) {
      if (!silent) error = `Update check failed: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      checkingUpdate = false;
    }
  }

  async function installUpdate() {
    if (!updateInfo?.available) return;
    installingUpdate = true;
    updateProgress = { downloaded: 0, total: undefined };
    error = null;
    info = null;
    try {
      await downloadAndInstallUpdate((d, t) => {
        updateProgress = { downloaded: d, total: t };
      });
      // If we reach here, relaunch() hasn't killed us yet — keep UI idle.
      info = 'Update installed. App will restart…';
    } catch (e) {
      error = `Update failed: ${e instanceof Error ? e.message : String(e)}`;
      installingUpdate = false;
      updateProgress = null;
    }
  }

  async function toggleResolution(id: string, enabled: boolean) {
    await withBusy(async () => {
      const s = await settingsApi.toggle(id, enabled);
      resolutions = s.resolutions;
    });
  }

  async function deleteCustomResolution(id: string) {
    if (!confirm('Delete this custom resolution?')) return;
    await withBusy(async () => {
      const s = await settingsApi.deleteCustom(id);
      resolutions = s.resolutions;
    });
  }

  async function saveResolutionsToProfile() {
    if (!activeProfileId) {
      error = 'No profile loaded. Save a profile first.';
      return;
    }
    const p = savedProfiles.find((x) => x.id === activeProfileId);
    if (!p) return;
    await withBusy(async () => {
      const saved = await profilesApi.save({
        id: p.id,
        name: p.name,
        gamePath: p.gamePath,
        gameUrl: p.gameUrl,
        gameSlug: p.gameSlug,
        resolutions
      });
      savedProfiles = await profilesApi.list();
      info = `Resolutions saved to "${saved.name}" (${resolutions.filter((r) => r.enabled).length}/${resolutions.length}).`;
    });
  }

  async function withBusy<T>(fn: () => Promise<T>): Promise<T | undefined> {
    busy = true;
    error = null;
    info = null;
    try {
      return await fn();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function installCa() {
    await withBusy(async () => {
      caState = await ca.install();
      info = caState.installed
        ? 'Local CA installed. Browsers will trust localhost HTTPS.'
        : 'CA install completed but verification failed.';
    });
  }

  async function uninstallCa() {
    await withBusy(async () => {
      caState = await ca.uninstall();
      info = 'Local CA uninstalled.';
    });
  }

  async function pickGame() {
    const dir = await pickFolder('Select your game folder (containing math/index.json)');
    if (!dir) return;
    await withBusy(async () => {
      game = await lgs.inspect(dir);
      activeProfileId = null; // new folder → no longer a saved profile
      if (status.running && status.math_dir !== game.mathDir) {
        status = await lgs.stop();
      }
    });
  }

  async function ensureLgsRunning() {
    if (!game) throw new Error('Pick a game folder first.');
    if (status.running && status.math_dir === game.mathDir) return;
    if (status.running) status = await lgs.stop();
    status = await lgs.start(DEFAULT_PORT, game.mathDir);
  }

  async function toggleLgs() {
    await withBusy(async () => {
      if (status.running) {
        status = await lgs.stop();
        info = 'LGS stopped.';
      } else {
        await ensureLgsRunning();
        info = `LGS listening on ${status.bound_addr}.`;
      }
    });
  }

  async function launch() {
    if (!game) {
      error = 'Pick a game folder first.';
      return;
    }
    if (!gameUrl) {
      error = 'Enter the front URL.';
      return;
    }
    await withBusy(async () => {
      await ensureLgsRunning();
      const params = new URLSearchParams({
        gameUrl,
        gameSlug: game!.slug,
        v: String(Date.now()) // cache-buster so browser always fetches fresh HTML
      });
      const port = (status.bound_addr ?? '').split(':').pop() ?? `${DEFAULT_PORT}`;
      const testUrl = `https://localhost:${port}/test/?${params.toString()}`;
      try {
        const r = await browser.openTest(testUrl);
        info = `Test view opened (${r.method}).`;
      } catch (e) {
        await openUrl(testUrl);
        info = 'Test view opened in default browser.';
      }
    });
  }

  // ---- Profiles ----

  async function loadProfile(p: Profile) {
    await withBusy(async () => {
      game = await lgs.inspect(p.gamePath);
      gameUrl = p.gameUrl;
      activeProfileId = p.id;
      if (status.running && status.math_dir !== game.mathDir) {
        status = await lgs.stop();
      }
      // Apply this profile's saved resolutions snapshot (if any).
      if (p.resolutions && p.resolutions.length > 0) {
        const s = await settingsApi.replace(p.resolutions);
        resolutions = s.resolutions;
      }
      info = `Loaded profile "${p.name}".`;
    });
  }

  function openSaveDialog() {
    if (!game) {
      error = 'Pick a game folder first.';
      return;
    }
    const existing = savedProfiles.find((p) => p.id === activeProfileId);
    saveName = existing?.name ?? game.slug;
    showSaveDialog = true;
  }

  async function saveProfile() {
    if (!game || !saveName.trim()) return;
    await withBusy(async () => {
      const saved = await profilesApi.save({
        id: activeProfileId ?? undefined,
        name: saveName.trim(),
        gamePath: game!.gamePath,
        gameUrl,
        gameSlug: game!.slug,
        resolutions  // capture current global resolutions snapshot
      });
      activeProfileId = saved.id;
      savedProfiles = await profilesApi.list();
      showSaveDialog = false;
      info = `Profile "${saved.name}" saved (with ${resolutions.filter((r) => r.enabled).length}/${resolutions.length} resolutions).`;
    });
  }

  async function deleteProfile(p: Profile) {
    if (!confirm(`Delete profile "${p.name}"?`)) return;
    await withBusy(async () => {
      await profilesApi.remove(p.id);
      if (activeProfileId === p.id) activeProfileId = null;
      savedProfiles = await profilesApi.list();
      info = `Profile "${p.name}" deleted.`;
    });
  }
</script>

<main class="min-h-screen p-8">
  <div class="mx-auto max-w-3xl">
    <header class="mb-8 flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Stake Dev Tool</h1>
        <p class="mt-1 text-sm text-zinc-400">Launch your slot against a local LGS</p>
      </div>
      <div class="flex items-center gap-2">
        <button
          onclick={() => checkUpdate(false)}
          disabled={checkingUpdate || installingUpdate}
          title="Check for updates"
          class="flex items-center gap-1.5 rounded-full border border-zinc-800 bg-zinc-900/60 px-3 py-1.5 text-xs text-zinc-400 transition hover:bg-zinc-800/60 disabled:opacity-50"
        >
          <svg class="h-3 w-3 {checkingUpdate ? 'animate-spin' : ''}" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h5M20 20v-5h-5M4 9a8 8 0 0114-3m2 8a8 8 0 01-14 3" />
          </svg>
          {checkingUpdate ? 'Checking…' : updateInfo?.available ? 'Update' : 'Check updates'}
        </button>
        <button
          onclick={toggleLgs}
          disabled={busy || (!status.running && !game)}
          class="flex items-center gap-2 rounded-full border border-zinc-800 bg-zinc-900/60 px-3 py-1.5 text-xs transition hover:bg-zinc-800/60 disabled:cursor-not-allowed disabled:opacity-60"
        >
          <span
            class="h-2 w-2 rounded-full {status.running
              ? 'bg-emerald-400 shadow-[0_0_8px_oklch(0.78_0.18_145)]'
              : 'bg-zinc-600'}"
          ></span>
          <span class="text-zinc-300">
            {status.running ? `LGS · ${status.bound_addr}` : 'LGS stopped'}
          </span>
        </button>
      </div>
    </header>

    {#if updateInfo?.available}
      <div class="mb-6 rounded-2xl border border-sky-900/60 bg-sky-950/20 p-4 backdrop-blur">
        <div class="flex items-start gap-3">
          <svg class="mt-0.5 h-5 w-5 flex-shrink-0 text-sky-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v12m0 0l-4-4m4 4l4-4m-9 8h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v13a2 2 0 002 2z" />
          </svg>
          <div class="flex-1">
            <div class="flex items-center gap-2 text-sm font-medium text-sky-200">
              Update available
              <span class="rounded bg-sky-500/20 px-2 py-0.5 font-mono text-[10px] text-sky-300">
                v{updateInfo.currentVersion} → v{updateInfo.version}
              </span>
            </div>
            {#if updateInfo.notes}
              <pre class="mt-2 max-h-32 overflow-y-auto whitespace-pre-wrap rounded bg-sky-950/40 p-2 text-[11px] text-sky-200/80">{updateInfo.notes}</pre>
            {/if}
            {#if installingUpdate && updateProgress}
              {@const pct = updateProgress.total
                ? Math.min(100, Math.round((updateProgress.downloaded / updateProgress.total) * 100))
                : null}
              <div class="mt-3">
                <div class="h-1.5 w-full overflow-hidden rounded bg-sky-950/60">
                  <div
                    class="h-full bg-sky-400 transition-all"
                    style="width: {pct ?? 30}%"
                  ></div>
                </div>
                <div class="mt-1 font-mono text-[10px] text-sky-300">
                  Downloading… {(updateProgress.downloaded / 1_048_576).toFixed(1)} MB
                  {#if updateProgress.total}
                    / {(updateProgress.total / 1_048_576).toFixed(1)} MB ({pct}%)
                  {/if}
                </div>
              </div>
            {:else}
              <button
                onclick={installUpdate}
                disabled={busy || installingUpdate}
                class="mt-3 rounded-md bg-sky-500 px-3 py-1.5 text-xs font-semibold text-zinc-950 transition hover:bg-sky-400 disabled:opacity-50"
              >
                Download &amp; install
              </button>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    {#if !caState.installed}
      <div class="mb-6 rounded-2xl border border-amber-900/60 bg-amber-950/20 p-4 backdrop-blur">
        <div class="flex items-start gap-3">
          <svg class="mt-0.5 h-5 w-5 flex-shrink-0 text-amber-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
          </svg>
          <div class="flex-1">
            <div class="text-sm font-medium text-amber-200">Trust local HTTPS certificates</div>
            <p class="mt-1 text-xs text-amber-200/70">
              The LGS serves over HTTPS with a self-signed certificate. Install our local Root CA
              into your Windows user trust store so browsers stop showing warnings. No UAC required.
            </p>
            <button
              onclick={installCa}
              disabled={busy}
              class="mt-3 rounded-md bg-amber-500 px-3 py-1.5 text-xs font-semibold text-zinc-950 transition hover:bg-amber-400 disabled:opacity-50"
            >
              Install Local CA
            </button>
          </div>
        </div>
      </div>
    {:else}
      <div class="mb-6 flex items-center justify-between rounded-2xl border border-emerald-900/40 bg-emerald-950/20 px-4 py-2.5 text-xs">
        <div class="flex items-center gap-2 text-emerald-300">
          <svg class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
            <path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm-2 16l-4-4 1.41-1.41L10 14.17l6.59-6.59L18 9l-8 8z" />
          </svg>
          Local CA installed — browsers trust localhost HTTPS
        </div>
        <button onclick={uninstallCa} disabled={busy} class="text-zinc-500 hover:text-zinc-300">
          Uninstall
        </button>
      </div>
    {/if}

    <!-- Profiles -->
    {#if savedProfiles.length > 0}
      <section class="mb-4">
        <div class="mb-2 flex items-center justify-between">
          <h2 class="text-[10px] font-medium uppercase tracking-wider text-zinc-500">
            Saved games
          </h2>
          <span class="text-[10px] text-zinc-600">{savedProfiles.length}</span>
        </div>
        <div class="flex flex-wrap gap-2">
          {#each savedProfiles as p (p.id)}
            {@const active = activeProfileId === p.id}
            <div
              class="group flex items-center gap-1 rounded-lg border px-2.5 py-1.5 text-xs transition {active
                ? 'border-emerald-500/40 bg-emerald-500/5 text-emerald-300'
                : 'border-zinc-800 bg-zinc-900/40 text-zinc-300 hover:border-zinc-700 hover:bg-zinc-900/80'}"
            >
              <button
                onclick={() => loadProfile(p)}
                disabled={busy}
                class="flex items-center gap-2 disabled:opacity-50"
              >
                <span class="font-medium">{p.name}</span>
                <span class="font-mono text-[10px] text-zinc-500">{p.gameSlug}</span>
              </button>
              <button
                onclick={() => deleteProfile(p)}
                disabled={busy}
                title="Delete profile"
                class="ml-1 rounded p-0.5 text-zinc-600 opacity-0 transition hover:bg-red-950/50 hover:text-red-400 group-hover:opacity-100"
              >
                <svg class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- Default resolutions -->
    <section class="mb-4 rounded-2xl border border-zinc-800/80 bg-zinc-900/40 backdrop-blur">
      <div class="flex w-full items-center justify-between px-5 py-3 text-xs font-medium uppercase tracking-wider text-zinc-400">
        <button
          type="button"
          onclick={() => (showResolutions = !showResolutions)}
          class="flex flex-1 items-center gap-2 text-left transition hover:text-zinc-200"
        >
          Default resolutions
          <span class="text-[10px] font-normal normal-case text-zinc-600">
            {resolutions.filter((r) => r.enabled).length}/{resolutions.length} enabled
          </span>
        </button>
        {#if activeProfileId}
          <button
            type="button"
            onclick={saveResolutionsToProfile}
            disabled={busy}
            class="mr-3 rounded-md border border-emerald-900/40 bg-emerald-950/30 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wider text-emerald-300 transition hover:bg-emerald-950/60 disabled:opacity-40"
          >
            Save
          </button>
        {/if}
        <button
          type="button"
          onclick={() => (showResolutions = !showResolutions)}
          aria-label="Toggle resolutions panel"
          class="text-zinc-500 transition hover:text-zinc-200"
        >
          <svg
            class="h-3.5 w-3.5 transition {showResolutions ? 'rotate-180' : ''}"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            viewBox="0 0 24 24"
          >
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
          </svg>
        </button>
      </div>
      {#if showResolutions}
        <div class="grid grid-cols-2 gap-1 border-t border-zinc-800/60 px-5 pb-4 pt-3">
          {#each resolutions as r (r.id)}
            <div
              class="group flex items-center gap-2 rounded px-2 py-1 transition hover:bg-zinc-800/40"
            >
              <input
                id="main-res-{r.id}"
                name="main-res-{r.id}"
                type="checkbox"
                checked={r.enabled}
                onchange={(e) => toggleResolution(r.id, (e.currentTarget as HTMLInputElement).checked)}
                disabled={busy}
                class="accent-emerald-500"
              />
              <label for="main-res-{r.id}" class="flex-1 cursor-pointer text-xs">
                <span class="text-zinc-100">{r.label}</span>
                <span class="ml-1 font-mono text-[10px] text-zinc-500">{r.width}×{r.height}</span>
                {#if !r.builtin}
                  <span
                    class="ml-1 rounded bg-amber-500/15 px-1 py-0.5 text-[9px] font-semibold text-amber-300"
                  >
                    custom
                  </span>
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
          <p class="col-span-2 mt-1 text-[10px] text-zinc-600">
            Add custom resolutions from the test view's sidebar.
          </p>
        </div>
      {/if}
    </section>

    <section class="rounded-2xl border border-zinc-800/80 bg-zinc-900/40 p-6 backdrop-blur">
      <!-- Step 1: game folder -->
      <div class="mb-6">
        <label for="main-math-folder" class="mb-1.5 block text-xs font-medium text-zinc-400">
          1 · Math folder of your game
        </label>
        <div class="flex gap-2">
          <input
            id="main-math-folder"
            name="main-math-folder"
            type="text"
            value={game?.gamePath ?? ''}
            readonly
            placeholder="No folder selected"
            class="flex-1 rounded-md border border-zinc-800 bg-zinc-950/60 px-3 py-2 font-mono text-sm placeholder:text-zinc-600 focus:outline-none"
          />
          <button
            onclick={pickGame}
            disabled={busy}
            class="rounded-md bg-zinc-100 px-4 py-2 text-xs font-semibold text-zinc-950 transition hover:bg-white disabled:opacity-50"
          >
            Browse…
          </button>
        </div>
        {#if game}
          <div class="mt-2 flex items-center gap-2 text-xs">
            <span class="rounded bg-emerald-500/10 px-2 py-0.5 font-medium text-emerald-400">
              {game.slug}
            </span>
            <span class="text-zinc-500">
              {game.modes.length} mode{game.modes.length === 1 ? '' : 's'}: {game.modes.join(', ') || '—'}
            </span>
          </div>
        {/if}
      </div>

      <!-- Step 2: front URL -->
      <div class="mb-6">
        <label for="main-front-url" class="mb-1.5 block text-xs font-medium text-zinc-400">
          2 · Front URL of your game
        </label>
        <input
          id="main-front-url"
          name="main-front-url"
          type="url"
          bind:value={gameUrl}
          placeholder="http://localhost:5174"
          class="w-full rounded-md border border-zinc-800 bg-zinc-950/60 px-3 py-2 font-mono text-sm placeholder:text-zinc-600 focus:border-emerald-500/50 focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
        />
      </div>

      <!-- Actions -->
      <div class="flex items-center gap-2">
        <button
          onclick={launch}
          disabled={busy || !game || !gameUrl}
          class="flex flex-1 items-center justify-center gap-2 rounded-md bg-emerald-500 px-4 py-2.5 text-sm font-semibold text-zinc-950 transition hover:bg-emerald-400 disabled:cursor-not-allowed disabled:opacity-40"
        >
          <svg class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
            <path d="M8 5v14l11-7z" />
          </svg>
          Launch test view
        </button>
        <button
          onclick={openSaveDialog}
          disabled={busy || !game}
          title={activeProfileId ? 'Update profile' : 'Save as profile'}
          class="rounded-md border border-zinc-800 bg-zinc-900 px-3 py-2.5 text-sm font-medium text-zinc-300 transition hover:bg-zinc-800 disabled:opacity-40"
        >
          {activeProfileId ? 'Update' : 'Save'}
        </button>
      </div>
    </section>

    <!-- Toasts -->
    <div class="mt-4 space-y-2">
      {#if info}
        <div class="rounded-md border border-emerald-900/60 bg-emerald-950/30 px-4 py-2 text-sm text-emerald-300">
          {info}
        </div>
      {/if}
      {#if error}
        <div class="rounded-md border border-red-900/60 bg-red-950/30 px-4 py-2 text-sm text-red-300">
          {error}
        </div>
      {/if}
    </div>
  </div>
</main>

<!-- Save profile dialog -->
{#if showSaveDialog}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) showSaveDialog = false;
    }}
    onkeydown={(e) => {
      if (e.key === 'Escape') showSaveDialog = false;
    }}
  >
    <div class="w-full max-w-sm rounded-xl border border-zinc-800 bg-zinc-900 p-5 shadow-2xl">
      <div class="mb-4">
        <div class="text-sm font-semibold text-zinc-100">
          {activeProfileId ? 'Update profile' : 'Save as profile'}
        </div>
        <div class="mt-0.5 text-xs text-zinc-500">
          Quick-load this math folder + front URL next time.
        </div>
      </div>
      <label for="profile-name" class="mb-1 block text-[10px] font-medium uppercase tracking-wider text-zinc-500">
        Name
      </label>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        id="profile-name"
        name="profile-name"
        type="text"
        bind:value={saveName}
        placeholder="e.g. easter-guardian-dev"
        autofocus
        onkeydown={(e) => {
          if (e.key === 'Enter') saveProfile();
        }}
        class="w-full rounded-md border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm focus:border-emerald-500/50 focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
      />
      <div class="mt-4 flex justify-end gap-2">
        <button
          onclick={() => (showSaveDialog = false)}
          class="rounded-md border border-zinc-800 bg-zinc-900 px-3 py-1.5 text-xs font-medium text-zinc-300 transition hover:bg-zinc-800"
        >
          Cancel
        </button>
        <button
          onclick={saveProfile}
          disabled={busy || !saveName.trim()}
          class="rounded-md bg-emerald-500 px-3 py-1.5 text-xs font-semibold text-zinc-950 transition hover:bg-emerald-400 disabled:opacity-40"
        >
          {activeProfileId ? 'Update' : 'Save'}
        </button>
      </div>
    </div>
  </div>
{/if}
