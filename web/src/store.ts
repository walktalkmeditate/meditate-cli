// Local-first persistence: one versioned localStorage blob holding prefs and a
// date-keyed completion map. No account, no backend — the streak lives only on
// this device. The streak math mirrors src/streak.rs (consecutive days, a gap
// resets, ≥60s earns a day); the day boundary is the LOCAL civil day (what a
// person expects), not the CLI's UTC day — a deliberate difference noted here.

const KEY = 'meditate.v1';
const SCHEMA_VERSION = 1;

/** A session must accrue this much active breathing to earn a day (parity with
 *  src/streak.rs MIN_SESSION_SECS). */
export const MIN_SESSION_SECS = 60;

export interface Prefs {
  pattern?: string;
  sound?: string;
  graphics?: boolean;
}

interface StoreData {
  schemaVersion: number;
  prefs: Prefs;
  completions: Record<string, true>; // "YYYY-MM-DD" (local) -> true
  totalSeconds: number;
}

export interface StreakStats {
  current: number;
  longest: number;
  rate30: number; // days completed within the last 30
  totalMinutes: number;
}

function emptyStore(): StoreData {
  return { schemaVersion: SCHEMA_VERSION, prefs: {}, completions: {}, totalSeconds: 0 };
}

/** A single absent/corrupt initializer — anything unreadable becomes a fresh
 *  store rather than blocking the app (a migration ladder waits for a v2). */
function loadStore(): StoreData {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return emptyStore();
    const parsed = JSON.parse(raw) as Partial<StoreData>;
    return {
      schemaVersion: SCHEMA_VERSION,
      prefs: parsed.prefs ?? {},
      completions: parsed.completions ?? {},
      totalSeconds: typeof parsed.totalSeconds === 'number' ? parsed.totalSeconds : 0,
    };
  } catch {
    return emptyStore();
  }
}

/** Local calendar date as `YYYY-MM-DD`. */
export function localDayKey(date = new Date()): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, '0');
  const d = String(date.getDate()).padStart(2, '0');
  return `${y}-${m}-${d}`;
}

/** A stable integer ordinal for a calendar date — used for consecutive-day math.
 *  Computed via Date.UTC on the calendar fields, so it is tz-independent. */
export function ordinalFromKey(key: string): number {
  const [y, m, d] = key.split('-').map(Number);
  return Math.floor(Date.UTC(y, m - 1, d) / 86_400_000);
}

export function todayOrdinal(date = new Date()): number {
  return Math.floor(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / 86_400_000);
}

/** Derive streak stats from a set of completed-day ordinals. Pure + tested. */
export function deriveStats(
  completedOrdinals: Set<number>,
  today: number,
  totalSeconds: number,
): StreakStats {
  // Current run: count back from today (or yesterday, if today isn't done yet).
  let anchor: number | null = null;
  if (completedOrdinals.has(today)) anchor = today;
  else if (completedOrdinals.has(today - 1)) anchor = today - 1;

  let current = 0;
  if (anchor !== null) {
    for (let d = anchor; completedOrdinals.has(d); d--) current++;
  }

  // Longest run anywhere.
  const sorted = [...completedOrdinals].sort((a, b) => a - b);
  let longest = 0;
  let run = 0;
  let prev: number | null = null;
  for (const o of sorted) {
    run = prev !== null && o === prev + 1 ? run + 1 : 1;
    longest = Math.max(longest, run);
    prev = o;
  }

  // 30-day rate: completions within the last 30 days (inclusive of today).
  let rate30 = 0;
  for (let d = today - 29; d <= today; d++) if (completedOrdinals.has(d)) rate30++;

  return { current, longest, rate30, totalMinutes: Math.floor(totalSeconds / 60) };
}

export class Persistence {
  private data: StoreData;
  private lastFlush = 0;

  constructor() {
    this.data = loadStore();
  }

  prefs(): Prefs {
    return this.data.prefs;
  }

  setPref<K extends keyof Prefs>(key: K, value: Prefs[K]): void {
    if (value === undefined || value === null) delete this.data.prefs[key];
    else this.data.prefs[key] = value;
    this.save();
  }

  /** Mark today complete (idempotent). */
  markToday(): void {
    const key = localDayKey();
    if (!this.data.completions[key]) {
      this.data.completions[key] = true;
      this.save();
    }
  }

  hasToday(): boolean {
    return this.data.completions[localDayKey()] === true;
  }

  /** Accrue breathing time; flushed at most every few seconds. */
  addSeconds(seconds: number, now: number): void {
    this.data.totalSeconds += seconds;
    if (now - this.lastFlush > 5000) {
      this.lastFlush = now;
      this.save();
    }
  }

  completedOrdinals(): Set<number> {
    return new Set(Object.keys(this.data.completions).map(ordinalFromKey));
  }

  stats(): StreakStats {
    return deriveStats(this.completedOrdinals(), todayOrdinal(), this.data.totalSeconds);
  }

  exportJson(): string {
    return JSON.stringify(this.data);
  }

  importJson(text: string): boolean {
    try {
      const parsed = JSON.parse(text) as Partial<StoreData>;
      if (typeof parsed !== 'object' || parsed === null) return false;
      this.data = {
        schemaVersion: SCHEMA_VERSION,
        prefs: parsed.prefs ?? {},
        completions: parsed.completions ?? {},
        totalSeconds: typeof parsed.totalSeconds === 'number' ? parsed.totalSeconds : 0,
      };
      this.save();
      return true;
    } catch {
      return false;
    }
  }

  private save(): void {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.data));
    } catch {
      /* storage full or blocked (private mode) — the session still runs */
    }
  }
}
