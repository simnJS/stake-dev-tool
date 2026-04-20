import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

export type LgsStatus = {
  running: boolean;
  bound_addr: string | null;
  math_dir: string | null;
};

export type GameInfo = {
  slug: string;
  path: string;
  modes: string[];
};

export type InspectedGame = {
  slug: string;
  gamePath: string;
  mathDir: string;
  modes: string[];
};

export type CaStatus = {
  installed: boolean;
  caPath: string;
};

export type LaunchOptions = {
  gameUrl: string;
  gameSlug: string;
  lang?: string;
  currency?: string;
  device?: string;
  social?: boolean;
  extraParams?: Array<[string, string]>;
};

export const lgs = {
  status: () => invoke<LgsStatus>('lgs_status'),
  start: (port: number, mathDir: string) =>
    invoke<LgsStatus>('start_lgs', { port, mathDir }),
  stop: () => invoke<LgsStatus>('stop_lgs'),
  listGames: (mathDir: string) => invoke<GameInfo[]>('list_games', { mathDir }),
  inspect: (path: string) => invoke<InspectedGame>('inspect_game_folder', { path }),
  launch: (options: LaunchOptions) => invoke<string>('launch_game', { options }),
  buildUrl: (options: LaunchOptions) => invoke<string>('build_launch_url', { options })
};

export const ca = {
  status: () => invoke<CaStatus>('ca_status'),
  install: () => invoke<CaStatus>('install_ca'),
  uninstall: () => invoke<CaStatus>('uninstall_ca')
};

export type PrepareSession = {
  sessionId: string;
  gameSlug: string;
  balance?: number;
  currency?: string;
  language?: string;
};

export const sessions = {
  prepare: (payload: PrepareSession) => invoke<void>('prepare_session', { payload })
};

export type OpenBrowserResult = { method: string; url: string };

export const browser = {
  openTest: (url: string) => invoke<OpenBrowserResult>('open_test_browser', { url })
};

// ===== Updater =====
// Thin wrappers around @tauri-apps/plugin-updater so the main page can show
// update status without pulling the plugin API everywhere.

export type UpdateInfo = {
  available: boolean;
  currentVersion: string;
  version?: string;
  notes?: string;
};

export async function checkForUpdates(): Promise<UpdateInfo> {
  const { check } = await import('@tauri-apps/plugin-updater');
  const { getVersion } = await import('@tauri-apps/api/app');
  const currentVersion = await getVersion();
  const update = await check();
  if (!update) return { available: false, currentVersion };
  return {
    available: true,
    currentVersion,
    version: update.version,
    notes: update.body
  };
}

export async function downloadAndInstallUpdate(
  onProgress?: (downloaded: number, total?: number) => void
): Promise<void> {
  const { check } = await import('@tauri-apps/plugin-updater');
  const { relaunch } = await import('@tauri-apps/plugin-process');
  const update = await check();
  if (!update) throw new Error('No update available');
  let downloaded = 0;
  let total: number | undefined;
  await update.downloadAndInstall((event) => {
    if (event.event === 'Started') {
      total = event.data.contentLength ?? undefined;
    } else if (event.event === 'Progress') {
      downloaded += event.data.chunkLength;
      onProgress?.(downloaded, total);
    }
  });
  await relaunch();
}

export type Profile = {
  id: string;
  name: string;
  gamePath: string;
  gameUrl: string;
  gameSlug: string;
  resolutions: ResolutionPreset[];
  createdAt: number;
  updatedAt: number;
};

export type SaveProfilePayload = {
  id?: string | null;
  name: string;
  gamePath: string;
  gameUrl: string;
  gameSlug: string;
  resolutions?: ResolutionPreset[];
};

export const profiles = {
  list: () => invoke<Profile[]>('list_profiles'),
  save: (payload: SaveProfilePayload) => invoke<Profile>('save_profile', { payload }),
  remove: (id: string) => invoke<void>('delete_profile', { id })
};

// ===== Settings (resolutions) =====

export type ResolutionPreset = {
  id: string;
  label: string;
  width: number;
  height: number;
  enabled: boolean;
  builtin: boolean;
};

export type Settings = { resolutions: ResolutionPreset[] };

// Tauri-side client (used by the desktop main page)
export const settings = {
  get: () => invoke<Settings>('get_settings'),
  toggle: (id: string, enabled: boolean) =>
    invoke<Settings>('toggle_resolution', { id, enabled }),
  addCustom: (label: string, width: number, height: number) =>
    invoke<Settings>('add_custom_resolution', { label, width, height }),
  deleteCustom: (id: string) => invoke<Settings>('delete_custom_resolution', { id }),
  replace: (resolutions: ResolutionPreset[]) =>
    invoke<Settings>('replace_resolutions', { resolutions })
};

// ---- Force event / last event / replay (HTTP, used by test view) ----

export type ForcedEvent = { mode: string; eventId: number };
export type ForcedEventStatus = { forced: ForcedEvent | null };
export type LastEvent = { eventId: number | null; payoutMultiplier: number | null };

export const forcedEventHttp = {
  get: async (): Promise<ForcedEventStatus> => {
    const r = await fetch('/api/devtool/force-event');
    if (!r.ok) throw new Error(`get force-event: ${r.status}`);
    return r.json();
  },
  set: async (mode: string, eventId: number): Promise<ForcedEventStatus> => {
    const r = await fetch('/api/devtool/force-event', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ mode, eventId })
    });
    if (!r.ok) {
      const t = await r.text();
      throw new Error(`set force-event: ${r.status} ${t}`);
    }
    return r.json();
  },
  clear: async (): Promise<ForcedEventStatus> => {
    const r = await fetch('/api/devtool/force-event', { method: 'DELETE' });
    if (!r.ok) throw new Error(`clear force-event: ${r.status}`);
    return r.json();
  }
};

export const lastEventHttp = {
  get: async (sessionId: string): Promise<LastEvent> => {
    const r = await fetch(`/api/devtool/sessions/${encodeURIComponent(sessionId)}/last-event`);
    if (!r.ok) throw new Error(`last-event: ${r.status}`);
    return r.json();
  }
};

export type EventEntry = {
  eventId: number;
  mode: string;
  betAmount: number;
  payout: number;
  payoutMultiplier: number;
  forced: boolean;
  at: number;
};

export type EventsHistory = { count: number; events: EventEntry[] };

export const historyHttp = {
  get: async (sessionId: string): Promise<EventsHistory> => {
    const r = await fetch(`/api/devtool/sessions/${encodeURIComponent(sessionId)}/events`);
    if (!r.ok) throw new Error(`events history: ${r.status}`);
    return r.json();
  }
};

export function replayUrl(
  gameUrl: string,
  gameSlug: string,
  lgsHostPort: string,
  opts: {
    mode: string;
    eventId: number;
    version?: string;
    currency?: string;
    amount?: number;
    lang?: string;
    device?: string;
    social?: boolean;
  }
): string {
  const u = new URL(gameUrl);
  u.searchParams.set('replay', 'true');
  u.searchParams.set('game', gameSlug);
  u.searchParams.set('version', opts.version ?? '1');
  u.searchParams.set('mode', opts.mode);
  u.searchParams.set('event', String(opts.eventId));
  u.searchParams.set('rgs_url', lgsHostPort);
  if (opts.currency) u.searchParams.set('currency', opts.currency);
  if (opts.amount !== undefined) u.searchParams.set('amount', String(opts.amount));
  if (opts.lang) u.searchParams.set('lang', opts.lang);
  if (opts.device) u.searchParams.set('device', opts.device);
  if (opts.social !== undefined) u.searchParams.set('social', opts.social ? 'true' : 'false');
  return u.toString();
}

// HTTP client (used by the test view served from LGS, no Tauri available)
export const settingsHttp = {
  get: async (): Promise<Settings> => {
    const r = await fetch('/api/devtool/settings');
    if (!r.ok) throw new Error(`get_settings: ${r.status}`);
    return r.json();
  },
  toggle: async (id: string, enabled: boolean): Promise<Settings> => {
    const r = await fetch('/api/devtool/settings/toggle', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, enabled })
    });
    if (!r.ok) throw new Error(`toggle: ${r.status}`);
    return r.json();
  },
  addCustom: async (label: string, width: number, height: number): Promise<Settings> => {
    const r = await fetch('/api/devtool/settings/custom', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ label, width, height })
    });
    if (!r.ok) {
      const t = await r.text();
      throw new Error(`addCustom: ${r.status} ${t}`);
    }
    return r.json();
  },
  deleteCustom: async (id: string): Promise<Settings> => {
    const r = await fetch(`/api/devtool/settings/custom/${encodeURIComponent(id)}`, {
      method: 'DELETE'
    });
    if (!r.ok) throw new Error(`deleteCustom: ${r.status}`);
    return r.json();
  }
};

export type Resolution = {
  id: string;
  label: string;
  width: number;
  height: number;
};

export const RESOLUTIONS: Resolution[] = [
  { id: 'desktop', label: 'Desktop', width: 1200, height: 675 },
  { id: 'laptop', label: 'Laptop', width: 1024, height: 576 },
  { id: 'popout-l', label: 'Popout L', width: 800, height: 450 },
  { id: 'popout-s', label: 'Popout S', width: 400, height: 225 },
  { id: 'mobile-l', label: 'Mobile L', width: 425, height: 821 },
  { id: 'mobile-m', label: 'Mobile M', width: 375, height: 667 },
  { id: 'mobile-s', label: 'Mobile S', width: 320, height: 568 }
];

// `country` is an ISO 3166-1 alpha-2 code used to fetch a flag SVG/PNG from
// flagcdn.com. `null` means no flag (fallback icon shown instead).
export type LanguageInfo = { code: string; name: string; country: string | null };
export type CurrencyInfo = {
  code: string;
  name: string;
  symbol: string;
  country: string | null;
  /** optional emoji/text fallback when no country flag fits (e.g. social tokens) */
  badge?: string;
};

export const LANGUAGES: LanguageInfo[] = [
  { code: 'ar', name: 'Arabic',     country: 'sa' },
  { code: 'de', name: 'German',     country: 'de' },
  { code: 'en', name: 'English',    country: 'gb' },
  { code: 'es', name: 'Spanish',    country: 'es' },
  { code: 'fi', name: 'Finnish',    country: 'fi' },
  { code: 'fr', name: 'French',     country: 'fr' },
  { code: 'hi', name: 'Hindi',      country: 'in' },
  { code: 'id', name: 'Indonesian', country: 'id' },
  { code: 'ja', name: 'Japanese',   country: 'jp' },
  { code: 'ko', name: 'Korean',     country: 'kr' },
  { code: 'pl', name: 'Polish',     country: 'pl' },
  { code: 'pt', name: 'Portuguese', country: 'pt' },
  { code: 'ru', name: 'Russian',    country: 'ru' },
  { code: 'tr', name: 'Turkish',    country: 'tr' },
  { code: 'vi', name: 'Vietnamese', country: 'vn' },
  { code: 'zh', name: 'Chinese',    country: 'cn' }
];

export const CURRENCIES: CurrencyInfo[] = [
  { code: 'USD', name: 'United States Dollar',       symbol: '$',    country: 'us' },
  { code: 'CAD', name: 'Canadian Dollar',            symbol: 'CA$',  country: 'ca' },
  { code: 'JPY', name: 'Japanese Yen',               symbol: '¥',    country: 'jp' },
  { code: 'EUR', name: 'Euro',                       symbol: '€',    country: 'eu' },
  { code: 'RUB', name: 'Russian Ruble',              symbol: '₽',    country: 'ru' },
  { code: 'CNY', name: 'Chinese Yuan',               symbol: 'CN¥',  country: 'cn' },
  { code: 'PHP', name: 'Philippine Peso',            symbol: '₱',    country: 'ph' },
  { code: 'INR', name: 'Indian Rupee',               symbol: '₹',    country: 'in' },
  { code: 'IDR', name: 'Indonesian Rupiah',          symbol: 'Rp',   country: 'id' },
  { code: 'KRW', name: 'South Korean Won',           symbol: '₩',    country: 'kr' },
  { code: 'BRL', name: 'Brazilian Real',             symbol: 'R$',   country: 'br' },
  { code: 'MXN', name: 'Mexican Peso',               symbol: 'MX$',  country: 'mx' },
  { code: 'DKK', name: 'Danish Krone',               symbol: 'KR',   country: 'dk' },
  { code: 'PLN', name: 'Polish Złoty',               symbol: 'zł',   country: 'pl' },
  { code: 'VND', name: 'Vietnamese Đồng',            symbol: '₫',    country: 'vn' },
  { code: 'TRY', name: 'Turkish Lira',               symbol: '₺',    country: 'tr' },
  { code: 'CLP', name: 'Chilean Peso',               symbol: 'CLP',  country: 'cl' },
  { code: 'ARS', name: 'Argentine Peso',             symbol: 'ARS',  country: 'ar' },
  { code: 'PEN', name: 'Peruvian Sol',               symbol: 'S/',   country: 'pe' },
  { code: 'NGN', name: 'Nigerian Naira',             symbol: '₦',    country: 'ng' },
  { code: 'SAR', name: 'Saudi Arabia Riyal',         symbol: 'SAR',  country: 'sa' },
  { code: 'ILS', name: 'Israel Shekel',              symbol: 'ILS',  country: 'il' },
  { code: 'AED', name: 'United Arab Emirates Dirham', symbol: 'AED', country: 'ae' },
  { code: 'TWD', name: 'Taiwan New Dollar',          symbol: 'NT$',  country: 'tw' },
  { code: 'NOK', name: 'Norway Krone',               symbol: 'kr',   country: 'no' },
  { code: 'KWD', name: 'Kuwaiti Dinar',              symbol: 'KD',   country: 'kw' },
  { code: 'JOD', name: 'Jordanian Dinar',            symbol: 'JD',   country: 'jo' },
  { code: 'CRC', name: 'Costa Rica Colon',           symbol: '₡',    country: 'cr' },
  { code: 'TND', name: 'Tunisian Dinar',             symbol: 'TND',  country: 'tn' },
  { code: 'SGD', name: 'Singapore Dollar',           symbol: 'SG$',  country: 'sg' },
  { code: 'MYR', name: 'Malaysia Ringgit',           symbol: 'RM',   country: 'my' },
  { code: 'OMR', name: 'Oman Rial',                  symbol: 'OMR',  country: 'om' },
  { code: 'QAR', name: 'Qatar Riyal',                symbol: 'QAR',  country: 'qa' },
  { code: 'BHD', name: 'Bahraini Dinar',             symbol: 'BD',   country: 'bh' },
  { code: 'XGC', name: 'Stake Gold Coin',            symbol: 'GC',   country: null, badge: 'GC' },
  { code: 'XSC', name: 'Stake Cash',                 symbol: 'SC',   country: null, badge: 'SC' }
];

export function flagUrl(country: string | null | undefined, height = 20): string | null {
  if (!country) return null;
  return `https://flagcdn.com/h${height}/${country}.png`;
}

export const API_MULTIPLIER = 1_000_000;

export async function pickFolder(title = 'Select math root folder'): Promise<string | null> {
  const result = await openDialog({
    title,
    directory: true,
    multiple: false
  });
  if (!result) return null;
  return Array.isArray(result) ? result[0] : result;
}
