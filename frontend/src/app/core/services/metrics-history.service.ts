import { Injectable, inject, signal } from '@angular/core';
import { EthNode, MetricSample } from '../models/node.model';
import { MockDataService } from './mock-data.service';
import { SettingsService } from './settings.service';

/** Max samples kept per node (~2 h at 3 s polling). */
const MAX_SAMPLES = 2400;

/**
 * Client-side time-series buffer built from successive GET /nodes responses.
 * The backend exposes no historical metrics endpoint yet, so live charts cover
 * the current browser session only (see README → missing endpoints).
 * Samples are keyed by the observation timestamp to avoid duplicating a poll
 * that returned the same observation twice.
 */
@Injectable({ providedIn: 'root' })
export class MetricsHistoryService {
  private readonly mock = inject(MockDataService);
  private readonly settings = inject(SettingsService);
  private readonly buffers = new Map<string, MetricSample[]>();

  /** Bumped on every write so computed() consumers re-read the buffers. */
  readonly version = signal(0);
  readonly sessionStart = Date.now();

  record(nodes: EthNode[], polledAt: number): void {
    let changed = false;
    for (const n of nodes) {
      const t = n.last_update ? new Date(n.last_update).getTime() : polledAt;
      const buf = this.buffers.get(n.node_id) ?? [];
      const last = buf[buf.length - 1];
      if (last && last.t >= t) continue;
      buf.push({ t, metrics: n.metrics });
      if (buf.length > MAX_SAMPLES) buf.splice(0, buf.length - MAX_SAMPLES);
      this.buffers.set(n.node_id, buf);
      changed = true;
    }
    if (changed) this.version.update((v) => v + 1);
  }

  clear(): void {
    this.buffers.clear();
    this.version.update((v) => v + 1);
  }

  /** Latest sample time across all nodes (live buffer only). */
  latestSampleTime(): number | null {
    let max: number | null = null;
    for (const buf of this.buffers.values()) {
      const t = buf[buf.length - 1]?.t;
      if (t !== undefined && (max === null || t > max)) max = t;
    }
    return max;
  }

  oldestSampleTime(): number | null {
    let min: number | null = null;
    for (const buf of this.buffers.values()) {
      const t = buf[0]?.t;
      if (t !== undefined && (min === null || t < min)) min = t;
    }
    return min;
  }

  /**
   * Samples of a node inside [end - range, end]. `end` is now, or the latest
   * sample when observations lag the wall clock (e.g. replayed datasets).
   */
  series(nodeId: string, rangeMs: number): MetricSample[] {
    if (this.settings.isDemo()) return this.mock.metricsHistory(nodeId, rangeMs);
    const buf = this.buffers.get(nodeId) ?? [];
    const end = this.windowEnd(rangeMs);
    return buf.filter((s) => s.t >= end - rangeMs && s.t <= end);
  }

  windowEnd(rangeMs: number): number {
    const now = Date.now();
    if (this.settings.isDemo()) return now;
    const latest = this.latestSampleTime();
    return latest !== null && latest < now - rangeMs ? latest : now;
  }

  /** True when live observation timestamps are behind the wall clock by more than the range. */
  isLagging(rangeMs: number): boolean {
    if (this.settings.isDemo()) return false;
    const latest = this.latestSampleTime();
    return latest !== null && latest < Date.now() - rangeMs;
  }
}
