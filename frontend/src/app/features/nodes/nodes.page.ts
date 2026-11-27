import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { Router, RouterLink } from '@angular/router';
import { NodeHealth, initialResource } from '../../core/models/common.model';
import { EthNode } from '../../core/models/node.model';
import { ClockService } from '../../core/services/clock.service';
import { NodeService } from '../../core/services/node.service';
import { readJson, writeJson } from '../../core/utils/storage';
import { HEALTH_LABEL } from '../../core/utils/labels';
import { ConfidenceComponent } from '../../shared/components/confidence/confidence';
import { CellDefDirective, DataTableComponent, TableColumn } from '../../shared/components/data-table/data-table';
import { IconComponent } from '../../shared/components/icon/icon';
import { LiveIndicatorComponent } from '../../shared/components/live-indicator/live-indicator';
import { StateComponent } from '../../shared/components/states/states';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { AgoPipe, MbPipe, MsPipe, NumPipe, PctPipe, Percent100Pipe } from '../../shared/pipes/format.pipes';

type ViewMode = 'grid' | 'table';
const VIEW_KEY = 'eva-guard.nodes-view';

@Component({
  selector: 'app-nodes-page',
  imports: [
    RouterLink,
    DataTableComponent,
    CellDefDirective,
    StatusBadgeComponent,
    ConfidenceComponent,
    StateComponent,
    IconComponent,
    LiveIndicatorComponent,
    AgoPipe,
    MbPipe,
    MsPipe,
    NumPipe,
    PctPipe,
    Percent100Pipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './nodes.page.html',
  styleUrl: './nodes.page.scss',
})
export class NodesPage {
  private readonly router = inject(Router);
  protected readonly clock = inject(ClockService);
  readonly res = toSignal(inject(NodeService).nodes$, { initialValue: initialResource<EthNode[]>('live') });

  readonly view = signal<ViewMode>(readJson<ViewMode>(VIEW_KEY, 'table'));
  readonly query = signal('');
  readonly statusFilter = signal<NodeHealth | 'all'>('all');
  readonly statuses: (NodeHealth | 'all')[] = ['all', 'healthy', 'warning', 'critical', 'offline'];
  readonly healthLabel = HEALTH_LABEL;

  readonly nodes = computed(() => this.res().data ?? []);
  readonly counts = computed(() => {
    const c: Record<string, number> = { all: this.nodes().length };
    for (const n of this.nodes()) c[n.status] = (c[n.status] ?? 0) + 1;
    return c;
  });
  readonly filtered = computed(() => {
    const q = this.query().trim().toLowerCase();
    const st = this.statusFilter();
    return this.nodes().filter((n) => (st === 'all' || n.status === st) && (!q || n.node_id.toLowerCase().includes(q)));
  });

  readonly columns: TableColumn<EthNode>[] = [
    { key: 'node_id', label: 'Node ID', sortable: true, mono: true },
    { key: 'status', label: 'Status', sortable: true, sortValue: (n) => ['critical', 'offline', 'warning', 'unknown', 'healthy'].indexOf(n.status) },
    { key: 'geth', label: 'Geth', sortable: true, sortValue: (n) => n.components.geth },
    { key: 'beacon', label: 'Beacon', sortable: true, sortValue: (n) => n.components.beacon, hideBelow: 'md' },
    { key: 'validator', label: 'Validator', sortable: true, sortValue: (n) => n.components.validator, hideBelow: 'md' },
    { key: 'cpu', label: 'CPU', align: 'right', sortable: true, mono: true, sortValue: (n) => n.metrics.cpu_usage_percent ?? null },
    { key: 'memory', label: 'Memory', align: 'right', sortable: true, mono: true, sortValue: (n) => n.metrics.memory_used_mb ?? null, hideBelow: 'lg' },
    { key: 'peers', label: 'Peers', align: 'right', sortable: true, mono: true, sortValue: (n) => n.metrics.peer_count ?? null },
    { key: 'latency', label: 'RPC latency', align: 'right', sortable: true, mono: true, sortValue: (n) => n.metrics.rpc_latency_ms ?? null, hideBelow: 'lg' },
    { key: 'block', label: 'Latest block', align: 'right', sortable: true, mono: true, sortValue: (n) => n.metrics.block_number ?? null, hideBelow: 'lg' },
    { key: 'ml', label: 'ML classification', sortable: true, sortValue: (n) => n.latest_prediction?.prediction ?? null },
  ];
  readonly nodeKey = (n: EthNode) => n.node_id;

  setView(v: ViewMode): void {
    this.view.set(v);
    writeJson(VIEW_KEY, v);
  }

  open(n: EthNode): void {
    this.router.navigate(['/nodes', n.node_id]);
  }

  memPct(n: EthNode): number | null {
    const m = n.metrics;
    return m.memory_used_mb != null && m.memory_total_mb ? (m.memory_used_mb / m.memory_total_mb) * 100 : null;
  }
}
