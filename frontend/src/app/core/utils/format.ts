/** Formatting helpers shared by pipes, charts and CSV export. All return '—' for missing values. */
export const NA = '—';

export function isNum(v: unknown): v is number {
  return typeof v === 'number' && Number.isFinite(v);
}

export function fmtNumber(v: number | null | undefined, digits = 0): string {
  if (!isNum(v)) return NA;
  return v.toLocaleString('en-US', { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

export function fmtCompact(v: number | null | undefined): string {
  if (!isNum(v)) return NA;
  return Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 }).format(v);
}

/** 0..1 → "96.4%". */
export function fmtPct(v: number | null | undefined, digits = 1): string {
  if (!isNum(v)) return NA;
  return `${(v * 100).toFixed(digits)}%`;
}

/** Already a percentage (0..100) → "42.0%". */
export function fmtPercentValue(v: number | null | undefined, digits = 1): string {
  if (!isNum(v)) return NA;
  return `${v.toFixed(digits)}%`;
}

export function fmtBytes(v: number | null | undefined, perSec = false): string {
  if (!isNum(v)) return NA;
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let n = Math.abs(v);
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  const s = `${(v < 0 ? -n : n).toFixed(n >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
  return perSec ? `${s}/s` : s;
}

export function fmtMb(v: number | null | undefined): string {
  if (!isNum(v)) return NA;
  return v >= 1024 ? `${(v / 1024).toFixed(1)} GB` : `${Math.round(v)} MB`;
}

export function fmtMs(v: number | null | undefined): string {
  if (!isNum(v)) return NA;
  if (v >= 1000) return `${(v / 1000).toFixed(2)} s`;
  return `${v < 10 ? v.toFixed(2) : v.toFixed(0)} ms`;
}

export function fmtDuration(seconds: number | null | undefined): string {
  if (!isNum(seconds)) return NA;
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ${m % 60}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}

export function timeAgo(iso: string | number | null | undefined, now = Date.now()): string {
  if (iso === null || iso === undefined || iso === '') return NA;
  const t = typeof iso === 'number' ? iso : new Date(iso).getTime();
  if (!Number.isFinite(t) || t <= 0) return NA;
  const diff = Math.round((now - t) / 1000);
  if (diff < -5) return 'in the future';
  if (diff < 5) return 'just now';
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

const pad = (n: number) => String(n).padStart(2, '0');

/** Local date-time "2026-11-25 09:16:04". */
export function fmtDateTime(iso: string | number | null | undefined, withDate = true): string {
  if (iso === null || iso === undefined || iso === '') return NA;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime()) || d.getTime() <= 0) return NA;
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
  return withDate ? `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${time}` : time;
}

export function fmtTime(iso: string | number | null | undefined): string {
  return fmtDateTime(iso, false);
}
