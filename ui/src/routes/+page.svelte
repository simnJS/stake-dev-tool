<script lang="ts">
  import { onMount } from 'svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as Sheet from '$lib/components/ui/sheet';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import { Switch } from '$lib/components/ui/switch';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Separator } from '$lib/components/ui/separator';
  import { Badge } from '$lib/components/ui/badge';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import { Toaster } from '$lib/components/ui/sonner';
  import { toast } from 'svelte-sonner';

  import PlayIcon from '@lucide/svelte/icons/play';
  import PlusIcon from '@lucide/svelte/icons/plus';
  import PencilIcon from '@lucide/svelte/icons/pencil';
  import TrashIcon from '@lucide/svelte/icons/trash-2';
  import FolderIcon from '@lucide/svelte/icons/folder';
  import RefreshIcon from '@lucide/svelte/icons/refresh-cw';
  import ShieldIcon from '@lucide/svelte/icons/shield-check';
  import ShieldAlertIcon from '@lucide/svelte/icons/shield-alert';
  import DownloadIcon from '@lucide/svelte/icons/download';
  import MonitorIcon from '@lucide/svelte/icons/monitor';
  import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
  import MinimizeIcon from '@lucide/svelte/icons/minimize-2';

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
  const LS_CLOSE_AFTER = 'sdt.closeAfterLaunch';

  let status = $state<LgsStatus>({ running: false, bound_addr: null, math_dir: null });
  let caState = $state<CaStatus>({ installed: false, caPath: '' });
  let updateInfo = $state<UpdateInfo | null>(null);
  let checkingUpdate = $state(false);
  let installingUpdate = $state(false);
  let updateProgress = $state<{ downloaded: number; total?: number } | null>(null);

  let savedProfiles = $state<Profile[]>([]);
  let activeProfileId = $state<string | null>(null);

  type Draft = {
    mode: 'new' | 'edit';
    id?: string;
    name: string;
    game: InspectedGame | null;
    gameUrl: string;
  };
  let draft = $state<Draft | null>(null);
  let drawerOpen = $state(false);

  let resolutions = $state<ResolutionPreset[]>([]);
  let showResolutions = $state(false);

  let closeAfterLaunch = $state(true);
  let busy = $state(false);

  onMount(() => {
    (async () => {
      try {
        const stored = localStorage.getItem(LS_CLOSE_AFTER);
        if (stored !== null) closeAfterLaunch = stored === '1';

        status = await lgs.status();
        caState = await ca.status();
        savedProfiles = await profilesApi.list();
        const s = await settingsApi.get();
        resolutions = s.resolutions;
      } catch (e) {
        console.error(e);
      }
      checkUpdate(true).catch(() => {});
    })();

    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) return;
      if (drawerOpen) return;
      if (e.key === 'Enter' && savedProfiles.length > 0) {
        const active = savedProfiles.find((p) => p.id === activeProfileId) ?? savedProfiles[0];
        launchProfile(active);
      } else if (e.key === 'n' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        openNewDraft();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  function persistCloseAfter() {
    localStorage.setItem(LS_CLOSE_AFTER, closeAfterLaunch ? '1' : '0');
  }

  async function withBusy<T>(fn: () => Promise<T>): Promise<T | undefined> {
    busy = true;
    try {
      return await fn();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      busy = false;
    }
  }

  function abbreviatePath(p: string, max = 52): string {
    if (p.length <= max) return p;
    const head = p.slice(0, 12);
    const tail = p.slice(p.length - (max - 14));
    return `${head}…${tail}`;
  }

  function formatRelative(ts: number | undefined): string {
    if (!ts) return '—';
    const diff = Date.now() - ts;
    if (diff < 60_000) return 'just now';
    if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
    if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
    const d = Math.floor(diff / 86_400_000);
    return d === 1 ? 'yesterday' : `${d}d ago`;
  }

  async function ensureLgsRunning(mathDir: string) {
    if (status.running && status.math_dir === mathDir) return;
    if (status.running) status = await lgs.stop();
    status = await lgs.start(DEFAULT_PORT, mathDir);
  }

  async function toggleLgs() {
    await withBusy(async () => {
      if (status.running) {
        status = await lgs.stop();
        toast.success('LGS stopped');
      } else {
        const p = savedProfiles.find((x) => x.id === activeProfileId) ?? savedProfiles[0];
        if (!p) throw new Error('Add a profile first.');
        const inspected = await lgs.inspect(p.gamePath);
        await ensureLgsRunning(inspected.mathDir);
        toast.success(`LGS listening on ${status.bound_addr}`);
      }
    });
  }

  async function checkUpdate(silent = false) {
    checkingUpdate = true;
    try {
      updateInfo = await checkForUpdates();
      if (!silent) {
        toast.success(
          updateInfo.available
            ? `Update available: v${updateInfo.version}`
            : `Up to date (v${updateInfo.currentVersion})`
        );
      }
    } catch (e) {
      if (!silent) toast.error(`Update check failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      checkingUpdate = false;
    }
  }

  async function installUpdate() {
    if (!updateInfo?.available) return;
    installingUpdate = true;
    updateProgress = { downloaded: 0, total: undefined };
    try {
      await downloadAndInstallUpdate((d, t) => {
        updateProgress = { downloaded: d, total: t };
      });
      toast.success('Update installed — restarting…');
    } catch (e) {
      toast.error(`Update failed: ${e instanceof Error ? e.message : String(e)}`);
      installingUpdate = false;
      updateProgress = null;
    }
  }

  async function installCa() {
    await withBusy(async () => {
      caState = await ca.install();
      if (caState.installed) toast.success('Local CA installed');
      else toast.error('CA install completed but verification failed');
    });
  }

  async function uninstallCa() {
    await withBusy(async () => {
      caState = await ca.uninstall();
      toast.success('Local CA uninstalled');
    });
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

  function openNewDraft() {
    draft = {
      mode: 'new',
      name: '',
      game: null,
      gameUrl: 'http://localhost:5174'
    };
    drawerOpen = true;
  }

  function openEditDraft(p: Profile) {
    draft = {
      mode: 'edit',
      id: p.id,
      name: p.name,
      game: null,
      gameUrl: p.gameUrl
    };
    drawerOpen = true;
    (async () => {
      try {
        const g = await lgs.inspect(p.gamePath);
        if (draft && draft.id === p.id) draft.game = g;
      } catch (e) {
        toast.error(e instanceof Error ? e.message : String(e));
      }
    })();
  }

  function closeDraft() {
    drawerOpen = false;
    setTimeout(() => {
      draft = null;
    }, 250);
  }

  async function pickDraftFolder() {
    if (!draft) return;
    const dir = await pickFolder('Select the math folder (containing index.json)');
    if (!dir) return;
    await withBusy(async () => {
      const g = await lgs.inspect(dir);
      if (!draft) return;
      draft.game = g;
      if (!draft.name.trim()) draft.name = g.slug;
    });
  }

  async function saveDraft() {
    if (!draft) return;
    if (!draft.game) return toast.error('Pick a math folder first');
    if (!draft.name.trim()) return toast.error('Give this profile a name');
    if (!draft.gameUrl.trim()) return toast.error('Enter the front URL');
    const d = draft;
    await withBusy(async () => {
      const saved = await profilesApi.save({
        id: d.mode === 'edit' ? d.id : undefined,
        name: d.name.trim(),
        gamePath: d.game!.gamePath,
        gameUrl: d.gameUrl.trim(),
        gameSlug: d.game!.slug,
        resolutions
      });
      activeProfileId = saved.id;
      savedProfiles = await profilesApi.list();
      closeDraft();
      toast.success(`Profile "${saved.name}" ${d.mode === 'edit' ? 'updated' : 'saved'}`);
    });
  }

  async function deleteProfile(p: Profile) {
    if (!confirm(`Delete profile "${p.name}"?`)) return;
    await withBusy(async () => {
      await profilesApi.remove(p.id);
      if (activeProfileId === p.id) activeProfileId = null;
      savedProfiles = await profilesApi.list();
      toast.success(`Deleted "${p.name}"`);
    });
  }

  async function launchProfile(p: Profile) {
    await withBusy(async () => {
      const inspected = await lgs.inspect(p.gamePath);
      await ensureLgsRunning(inspected.mathDir);

      if (p.resolutions && p.resolutions.length > 0) {
        const s = await settingsApi.replace(p.resolutions);
        resolutions = s.resolutions;
      }

      const params = new URLSearchParams({
        gameUrl: p.gameUrl,
        gameSlug: inspected.slug,
        v: String(Date.now())
      });
      const port = (status.bound_addr ?? '').split(':').pop() ?? `${DEFAULT_PORT}`;
      const testUrl = `https://localhost:${port}/test/?${params.toString()}`;

      try {
        const r = await browser.openTest(testUrl);
        toast.success(`Opened test view (${r.method})`);
      } catch {
        await openUrl(testUrl);
        toast.success('Opened in default browser');
      }

      activeProfileId = p.id;

      if (closeAfterLaunch) {
        await new Promise((r) => setTimeout(r, 500));
        try {
          await getCurrentWindow().minimize();
        } catch {
          // ignore
        }
      }
    });
  }

  async function saveResolutionsToActiveProfile() {
    const id = activeProfileId ?? savedProfiles[0]?.id;
    if (!id) return toast.error('Add a profile first');
    const p = savedProfiles.find((x) => x.id === id);
    if (!p) return;
    await withBusy(async () => {
      await profilesApi.save({
        id: p.id,
        name: p.name,
        gamePath: p.gamePath,
        gameUrl: p.gameUrl,
        gameSlug: p.gameSlug,
        resolutions
      });
      savedProfiles = await profilesApi.list();
      toast.success(`Snapshot saved to "${p.name}"`);
    });
  }

  const enabledResCount = $derived(resolutions.filter((r) => r.enabled).length);
  const hasProfiles = $derived(savedProfiles.length > 0);
  const currentVersion = $derived(updateInfo?.currentVersion ?? '');
</script>

<svelte:head>
  <title>Stake Dev Tool</title>
</svelte:head>

<Toaster position="top-right" richColors closeButton />

<main class="mx-auto flex min-h-screen w-full max-w-4xl flex-col gap-8 px-8 py-10">
  <!-- Topbar -->
  <header class="flex items-center justify-between">
    <div class="flex items-center gap-4">
      <div class="flex h-10 w-10 items-center justify-center rounded-xl border bg-card text-lg font-semibold">
        S
      </div>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Stake Dev Tool</h1>
        <p class="text-sm text-muted-foreground">
          {#if currentVersion}
            <span class="font-mono-tab">v{currentVersion}</span> ·
          {/if}
          Slot game workbench
        </p>
      </div>
    </div>

    <div class="flex items-center gap-2">
      <Tooltip.Provider delayDuration={200}>
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <Button
                {...props}
                variant="outline"
                size="lg"
                onclick={toggleLgs}
                disabled={busy || (!status.running && !hasProfiles)}
              >
                {#if status.running}
                  <span class="status-dot status-dot-live pulse-gentle"></span>
                  <span class="font-mono-tab text-sm">{status.bound_addr}</span>
                {:else}
                  <span class="status-dot status-dot-off"></span>
                  <span class="text-sm">LGS offline</span>
                {/if}
              </Button>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content>{status.running ? 'Click to stop' : 'Click to start'}</Tooltip.Content>
        </Tooltip.Root>

        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <Button
                {...props}
                variant="ghost"
                size="icon-lg"
                onclick={() => checkUpdate(false)}
                disabled={checkingUpdate || installingUpdate}
              >
                <RefreshIcon class={checkingUpdate ? 'animate-spin' : ''} />
              </Button>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content>Check for updates</Tooltip.Content>
        </Tooltip.Root>
      </Tooltip.Provider>
    </div>
  </header>

  <!-- Update banner -->
  {#if updateInfo?.available}
    <Card.Root class="fade-in border-blue-500/30 bg-blue-500/5">
      <Card.Content class="flex items-start gap-4 pt-6">
        <div class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-lg bg-blue-500/10 text-blue-400">
          <DownloadIcon class="h-5 w-5" />
        </div>
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-3">
            <h3 class="text-base font-semibold">Update available</h3>
            <Badge variant="secondary" class="font-mono-tab">
              v{updateInfo.currentVersion} → v{updateInfo.version}
            </Badge>
          </div>
          {#if updateInfo.notes}
            <pre class="mt-3 max-h-32 overflow-y-auto whitespace-pre-wrap rounded-md border bg-background p-3 font-mono text-xs leading-relaxed text-muted-foreground">{updateInfo.notes}</pre>
          {/if}
          {#if installingUpdate && updateProgress}
            {@const pct = updateProgress.total
              ? Math.min(100, Math.round((updateProgress.downloaded / updateProgress.total) * 100))
              : null}
            <div class="mt-4">
              <div class="h-2 w-full overflow-hidden rounded-full bg-muted">
                <div class="h-full bg-blue-500 transition-all" style="width: {pct ?? 30}%"></div>
              </div>
              <div class="mt-2 font-mono-tab text-xs text-muted-foreground">
                {(updateProgress.downloaded / 1_048_576).toFixed(1)} MB
                {#if updateProgress.total}
                  / {(updateProgress.total / 1_048_576).toFixed(1)} MB ({pct}%)
                {/if}
              </div>
            </div>
          {:else}
            <Button size="lg" class="mt-4" onclick={installUpdate} disabled={busy || installingUpdate}>
              Download &amp; install
            </Button>
          {/if}
        </div>
      </Card.Content>
    </Card.Root>
  {/if}

  <!-- CA setup -->
  {#if !caState.installed}
    <Card.Root class="fade-in border-amber-500/30 bg-amber-500/5">
      <Card.Content class="flex items-start gap-4 pt-6">
        <div class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-lg bg-amber-500/10 text-amber-400">
          <ShieldAlertIcon class="h-5 w-5" />
        </div>
        <div class="flex-1">
          <h3 class="text-base font-semibold">Trust local HTTPS</h3>
          <p class="mt-1.5 text-sm leading-relaxed text-muted-foreground">
            Install the local Root CA so browsers trust <code class="font-mono-tab text-foreground">localhost</code>
            HTTPS. No UAC; user-scope only.
          </p>
          <Button size="lg" class="mt-4" onclick={installCa} disabled={busy}>
            <ShieldIcon />
            Install Local CA
          </Button>
        </div>
      </Card.Content>
    </Card.Root>
  {/if}

  <!-- Games -->
  <section class="flex flex-col gap-5">
    <div class="flex items-center justify-between">
      <div>
        <h2 class="text-xl font-semibold tracking-tight">Games</h2>
        <p class="mt-1 text-sm text-muted-foreground">
          {#if hasProfiles}
            Click a profile to launch. Press
            <span class="kbd">↵</span>
            to launch the active one, or
            <span class="kbd">⌘</span><span class="kbd">N</span>
            to add a new game.
          {:else}
            Add your first game below.
          {/if}
        </p>
      </div>
      {#if hasProfiles}
        <Button size="lg" onclick={openNewDraft}>
          <PlusIcon />
          New game
        </Button>
      {/if}
    </div>

    {#if !hasProfiles}
      <!-- Empty state -->
      <Card.Root class="border-dashed">
        <Card.Content class="flex flex-col items-start gap-4 py-12">
          <div class="flex h-12 w-12 items-center justify-center rounded-xl border bg-muted text-muted-foreground">
            <FolderIcon class="h-5 w-5" />
          </div>
          <div>
            <h3 class="text-lg font-semibold">Pin your first game</h3>
            <p class="mt-1.5 max-w-md text-sm leading-relaxed text-muted-foreground">
              Point us at a math folder and a front URL. We'll keep them pinned so one click
              fires up the whole local test matrix.
            </p>
          </div>
          <Button size="lg" onclick={openNewDraft}>
            <PlusIcon />
            Add a game
          </Button>
        </Card.Content>
      </Card.Root>
    {:else}
      <!-- Profile cards -->
      <div class="flex flex-col gap-3">
        {#each savedProfiles as p, i (p.id)}
          {@const active = activeProfileId === p.id}
          <Card.Root
            class="group fade-in relative overflow-hidden transition hover:border-foreground/25 {active
              ? 'border-foreground/40'
              : ''}"
            style="animation-delay: {i * 40}ms;"
          >
            {#if active}
              <span class="absolute left-0 top-4 bottom-4 w-[3px] rounded-r bg-foreground/80"></span>
            {/if}
            <Card.Content class="flex items-center gap-4 py-4 pl-6 pr-4">
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-3">
                  <button
                    type="button"
                    class="text-left text-lg font-semibold tracking-tight transition hover:text-foreground/90"
                    onclick={() => launchProfile(p)}
                    disabled={busy}
                  >
                    {p.name}
                  </button>
                  <Badge variant="secondary" class="font-mono-tab text-xs">{p.gameSlug}</Badge>
                  {#if active}
                    <Badge variant="outline" class="gap-1.5 text-xs">
                      <span class="status-dot status-dot-live"></span>
                      active
                    </Badge>
                  {/if}
                </div>
                <div class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground font-mono-tab">
                  <span class="flex items-center gap-1.5" title={p.gamePath}>
                    <FolderIcon class="h-3.5 w-3.5" />
                    {abbreviatePath(p.gamePath, 46)}
                  </span>
                  <span class="flex items-center gap-1.5" title={p.gameUrl}>
                    <MonitorIcon class="h-3.5 w-3.5" />
                    {p.gameUrl}
                  </span>
                  <span>· updated {formatRelative(p.updatedAt)}</span>
                </div>
              </div>

              <div class="flex items-center gap-1">
                <Tooltip.Provider delayDuration={200}>
                  <Tooltip.Root>
                    <Tooltip.Trigger>
                      {#snippet child({ props })}
                        <Button
                          {...props}
                          variant="ghost"
                          size="icon"
                          class="opacity-0 transition group-hover:opacity-100"
                          onclick={() => openEditDraft(p)}
                          disabled={busy}
                        >
                          <PencilIcon />
                        </Button>
                      {/snippet}
                    </Tooltip.Trigger>
                    <Tooltip.Content>Edit</Tooltip.Content>
                  </Tooltip.Root>

                  <Tooltip.Root>
                    <Tooltip.Trigger>
                      {#snippet child({ props })}
                        <Button
                          {...props}
                          variant="ghost"
                          size="icon"
                          class="text-destructive opacity-0 transition hover:text-destructive group-hover:opacity-100"
                          onclick={() => deleteProfile(p)}
                          disabled={busy}
                        >
                          <TrashIcon />
                        </Button>
                      {/snippet}
                    </Tooltip.Trigger>
                    <Tooltip.Content>Delete</Tooltip.Content>
                  </Tooltip.Root>
                </Tooltip.Provider>

                <Button size="lg" onclick={() => launchProfile(p)} disabled={busy}>
                  <PlayIcon class="h-4 w-4" />
                  Launch
                </Button>
              </div>
            </Card.Content>
          </Card.Root>
        {/each}
      </div>
    {/if}
  </section>

  <!-- Resolutions -->
  <section class="flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <div>
        <h2 class="text-xl font-semibold tracking-tight">Resolutions</h2>
        <p class="mt-1 text-sm text-muted-foreground">
          <span class="font-mono-tab text-foreground">{enabledResCount}/{resolutions.length}</span> enabled ·
          applied per launch.
        </p>
      </div>
      <div class="flex items-center gap-2">
        {#if activeProfileId}
          <Button variant="outline" size="lg" onclick={saveResolutionsToActiveProfile} disabled={busy}>
            Snapshot to profile
          </Button>
        {/if}
        <Button variant="ghost" size="lg" onclick={() => (showResolutions = !showResolutions)}>
          {showResolutions ? 'Hide' : 'Show'}
          <ChevronDownIcon class="transition {showResolutions ? 'rotate-180' : ''}" />
        </Button>
      </div>
    </div>

    {#if showResolutions}
      <Card.Root class="fade-in">
        <Card.Content class="grid grid-cols-1 gap-1 p-2 sm:grid-cols-2">
          {#each resolutions as r (r.id)}
            <label
              for="res-{r.id}"
              class="group flex cursor-pointer items-center gap-3 rounded-md px-3 py-3 transition hover:bg-muted/50"
            >
              <Checkbox
                id="res-{r.id}"
                checked={r.enabled}
                onCheckedChange={(v) => toggleResolution(r.id, Boolean(v))}
                disabled={busy}
              />
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="text-sm font-medium">{r.label}</span>
                  {#if !r.builtin}
                    <Badge variant="outline" class="text-[10px]">custom</Badge>
                  {/if}
                </div>
                <div class="font-mono-tab text-xs text-muted-foreground">
                  {r.width} × {r.height}
                </div>
              </div>
              {#if !r.builtin}
                <Button
                  variant="ghost"
                  size="icon-sm"
                  class="text-destructive opacity-0 transition group-hover:opacity-100"
                  onclick={(e) => {
                    e.preventDefault();
                    deleteCustomResolution(r.id);
                  }}
                  disabled={busy}
                >
                  <TrashIcon />
                </Button>
              {/if}
            </label>
          {/each}
        </Card.Content>
      </Card.Root>
    {/if}
  </section>

  <!-- Footer -->
  <footer class="mt-auto flex flex-wrap items-center justify-between gap-6 border-t pt-6 text-sm">
    <div class="flex items-center gap-3">
      <Switch id="close-after" bind:checked={closeAfterLaunch} onCheckedChange={persistCloseAfter} />
      <Label for="close-after" class="flex items-center gap-2 text-sm font-normal">
        <MinimizeIcon class="h-4 w-4 text-muted-foreground" />
        Minimise window after launching
      </Label>
    </div>

    <div class="flex items-center gap-4 text-muted-foreground">
      {#if caState.installed}
        <span class="flex items-center gap-2">
          <ShieldIcon class="h-4 w-4 text-emerald-500" />
          <span>CA trusted</span>
          <button
            type="button"
            class="underline-offset-4 hover:text-foreground hover:underline"
            onclick={uninstallCa}
            disabled={busy}
          >
            remove
          </button>
        </span>
      {/if}
    </div>
  </footer>
</main>

<!-- Drawer: create/edit profile -->
<Sheet.Root bind:open={drawerOpen}>
  <Sheet.Content side="right" class="w-full !max-w-[480px] flex flex-col gap-0 p-0">
    {#if draft}
      <Sheet.Header class="border-b px-6 py-5">
        <Sheet.Title class="text-xl">
          {draft.mode === 'edit' ? 'Edit profile' : 'Add a game'}
        </Sheet.Title>
        <Sheet.Description>
          {draft.mode === 'edit'
            ? 'Update the math folder, URL or name.'
            : 'Point us at a math folder and a front URL. We\'ll pin it.'}
        </Sheet.Description>
      </Sheet.Header>

      <div class="flex-1 overflow-y-auto px-6 py-5 flex flex-col gap-6">
        <div class="flex flex-col gap-2">
          <Label for="draft-name" class="text-sm">Profile name</Label>
          <Input
            id="draft-name"
            type="text"
            bind:value={draft.name}
            placeholder="e.g. easter-guardian-dev"
            class="h-10"
          />
        </div>

        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between">
            <Label for="draft-path" class="text-sm">Math folder</Label>
            {#if draft.game}
              <span class="font-mono-tab text-xs text-muted-foreground">
                {draft.game.modes.length} mode{draft.game.modes.length === 1 ? '' : 's'}
              </span>
            {/if}
          </div>
          <div class="flex gap-2">
            <Input
              id="draft-path"
              type="text"
              value={draft.game?.gamePath ?? ''}
              readonly
              placeholder="No folder selected"
              class="h-10 font-mono-tab text-sm"
            />
            <Button variant="outline" size="lg" onclick={pickDraftFolder} disabled={busy}>
              <FolderIcon />
              Browse
            </Button>
          </div>
          {#if draft.game}
            <div class="mt-1 flex flex-wrap gap-1.5">
              <Badge variant="secondary" class="font-mono-tab text-xs">{draft.game.slug}</Badge>
              {#each draft.game.modes as m (m)}
                <Badge variant="outline" class="font-mono-tab text-xs">{m}</Badge>
              {/each}
            </div>
          {/if}
        </div>

        <div class="flex flex-col gap-2">
          <Label for="draft-url" class="text-sm">Front URL</Label>
          <Input
            id="draft-url"
            type="url"
            bind:value={draft.gameUrl}
            placeholder="http://localhost:5174"
            class="h-10 font-mono-tab text-sm"
          />
          <p class="text-xs text-muted-foreground">
            Your game's frontend dev server — usually a Vite URL.
          </p>
        </div>

        <Separator />

        <div class="rounded-lg border bg-muted/30 p-4">
          <div class="text-sm">
            Saving will pin
            <span class="font-mono-tab font-semibold">{enabledResCount}/{resolutions.length}</span>
            resolutions onto this profile. Re-snapshot any time from the main view.
          </div>
        </div>
      </div>

      <Sheet.Footer class="flex-row justify-end gap-2 border-t px-6 py-4">
        <Button variant="outline" size="lg" onclick={closeDraft}>Cancel</Button>
        <Button
          size="lg"
          onclick={saveDraft}
          disabled={busy || !draft.game || !draft.name.trim() || !draft.gameUrl.trim()}
        >
          {draft.mode === 'edit' ? 'Save changes' : 'Create profile'}
        </Button>
      </Sheet.Footer>
    {/if}
  </Sheet.Content>
</Sheet.Root>
