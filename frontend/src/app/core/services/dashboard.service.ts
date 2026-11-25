import { Injectable, inject } from '@angular/core';
import { normalizeDistribution, normalizeSummary, normalizeTimeline } from '../api/normalizers';
import { DashboardSummary, DistributionEntry, TimelinePoint } from '../models/dashboard.model';
import { API_ENDPOINTS, ApiService } from './api.service';
import { MockDataService } from './mock-data.service';
import { PollingService } from './polling.service';

/** GET /api/v1/dashboard/{summary,distribution,timeline} — polled every interval (3 s by default). */
@Injectable({ providedIn: 'root' })
export class DashboardService {
  private readonly api = inject(ApiService);
  private readonly polling = inject(PollingService);
  private readonly mock = inject(MockDataService);

  readonly summary$ = this.polling.poll<DashboardSummary>({
    live: () => this.api.get(API_ENDPOINTS.dashboardSummary),
    normalize: normalizeSummary,
    demo: () => this.mock.summary(),
  });

  readonly distribution$ = this.polling.poll<DistributionEntry[]>({
    live: () => this.api.get(API_ENDPOINTS.dashboardDistribution),
    normalize: normalizeDistribution,
    demo: () => this.mock.distribution(),
  });

  readonly timeline$ = this.polling.poll<TimelinePoint[]>({
    live: () => this.api.get(API_ENDPOINTS.dashboardTimeline),
    normalize: normalizeTimeline,
    demo: () => this.mock.timeline(),
  });
}
