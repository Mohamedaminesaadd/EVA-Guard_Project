import { ChangeDetectionStrategy, Component, VERSION, computed, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { concatMap, from, toArray } from 'rxjs';
import { environment } from '../../../environments/environment';
import { initialResource } from '../../core/models/common.model';
import { EthNode } from '../../core/models/node.model';
import { AgentService } from '../../core/services/agent.service';
import { AlertService } from '../../core/services/alert.service';
import { API_ENDPOINTS, ApiService, EndpointProbe } from '../../core/services/api.service';
import { ConnectionService } from '../../core/services/connection.service';
import { InvestigationStatusService } from '../../core/services/investigation-status.service';
import { MetricsHistoryService } from '../../core/services/metrics-history.service';
import { NodeService } from '../../core/services/node.service';
import { SettingsService } from '../../core/services/settings.service';
import { IconComponent } from '../../shared/components/icon/icon';
import { MsPipe } from '../../shared/pipes/format.pipes';

@Component({
  selector: 'app-settings-page',
  imports: [IconComponent, MsPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './settings.page.html',
  styleUrl: './settings.page.scss',
})
export class SettingsPage {
  protected readonly settings = inject(SettingsService);
  protected readonly connection = inject(ConnectionService);
  protected readonly agent = inject(AgentService);
  private readonly api = inject(ApiService);
  private readonly alerts = inject(AlertService);
  private readonly investigation = inject(InvestigationStatusService);
  private readonly history = inject(MetricsHistoryService);
  private readonly nodes = toSignal(inject(NodeService).nodes$, { initialValue: initialResource<EthNode[]>('live') });

  readonly env = environment;
  readonly angularVersion = VERSION.full;
  readonly intervals = [3000, 5000, 10000, 30000];
  readonly urlDraft = signal(this.settings.apiBaseUrlOverride() ?? '');
  readonly probing = signal(false);
  readonly probes = signal<EndpointProbe[]>([]);
  readonly cleared = signal<string | null>(null);

  readonly sampleNode = computed(() => this.nodes().data?.[0]?.node_id ?? 'node-01');

  readonly endpoints = computed(() => [
    API_ENDPOINTS.health,
    API_ENDPOINTS.status,
    API_ENDPOINTS.nodes,
    API_ENDPOINTS.node(this.sampleNode()),
    API_ENDPOINTS.predictionsLatest,
    API_ENDPOINTS.predictions,
    API_ENDPOINTS.predictionsByNode(this.sampleNode()),
    API_ENDPOINTS.alerts,
    API_ENDPOINTS.dashboardSummary,
    API_ENDPOINTS.dashboardDistribution,
    API_ENDPOINTS.dashboardTimeline,
    API_ENDPOINTS.dashboardOverview,
    API_ENDPOINTS.monitoringLatest,
    API_ENDPOINTS.incidents,
    API_ENDPOINTS.investigations,
  ]);

  readonly probeSummary = computed(() => {
    const p = this.probes();
    return p.length ? `${p.filter((x) => x.ok).length}/${p.length} endpoints OK` : '';
  });

  applyUrl(): void {
    const v = this.urlDraft().trim();
    this.settings.apiBaseUrlOverride.set(v && v !== environment.apiBaseUrl ? v.replace(/\/+$/, '') : null);
    this.probes.set([]);
  }

  resetUrl(): void {
    this.urlDraft.set('');
    this.settings.apiBaseUrlOverride.set(null);
  }

  testEndpoints(): void {
    this.probing.set(true);
    this.probes.set([]);
    from(this.endpoints())
      .pipe(
        concatMap((path) => this.api.probe(path)),
        toArray(),
      )
      .subscribe((res) => {
        this.probes.set(res);
        this.probing.set(false);
      });
  }

  clear(what: 'alerts' | 'investigations' | 'conversations' | 'buffer' | 'settings'): void {
    switch (what) {
      case 'alerts':
        this.alerts.clearLocalState();
        break;
      case 'investigations':
        this.investigation.clear();
        break;
      case 'conversations':
        this.agent.clearAll();
        break;
      case 'buffer':
        this.history.clear();
        break;
      case 'settings':
        this.settings.reset();
        this.urlDraft.set('');
        break;
    }
    this.cleared.set(what);
    setTimeout(() => this.cleared.set(null), 2500);
  }
}
