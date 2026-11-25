import {
  ChangeDetectionStrategy,
  Component,
  ElementRef,
  computed,
  effect,
  inject,
  signal,
  untracked,
  viewChild,
} from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { SecurityAlert } from '../../core/models/alert.model';
import { Investigation } from '../../core/models/agent.model';
import { initialResource } from '../../core/models/common.model';
import { EthNode } from '../../core/models/node.model';
import { AGENT_PROFILE, AgentService, SUGGESTED_PROMPTS } from '../../core/services/agent.service';
import { AlertService } from '../../core/services/alert.service';
import { ClockService } from '../../core/services/clock.service';
import { NodeService } from '../../core/services/node.service';
import { SettingsService } from '../../core/services/settings.service';
import { ATTACK_LABEL, SEVERITY_LABEL } from '../../core/utils/labels';
import { ConfidenceComponent, ProbBarComponent } from '../../shared/components/confidence/confidence';
import { IconComponent } from '../../shared/components/icon/icon';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { AgoPipe, MbPipe, MsPipe, NumPipe, PctPipe, Percent100Pipe } from '../../shared/pipes/format.pipes';
import { MarkdownPipe } from './markdown.pipe';

@Component({
  selector: 'app-agent-page',
  imports: [
    RouterLink,
    IconComponent,
    StatusBadgeComponent,
    ConfidenceComponent,
    ProbBarComponent,
    MarkdownPipe,
    AgoPipe,
    PctPipe,
    Percent100Pipe,
    MbPipe,
    MsPipe,
    NumPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './agent.page.html',
  styleUrl: './agent.page.scss',
})
export class AgentPage {
  protected readonly agent = inject(AgentService);
  protected readonly settings = inject(SettingsService);
  protected readonly clock = inject(ClockService);
  private readonly route = inject(ActivatedRoute);

  private readonly nodesRes = toSignal(inject(NodeService).nodes$, { initialValue: initialResource<EthNode[]>('live') });
  private readonly alertsRes = toSignal(inject(AlertService).alerts$, { initialValue: initialResource<SecurityAlert[]>('live') });

  private readonly thread = viewChild<ElementRef<HTMLElement>>('thread');
  private readonly inputEl = viewChild<ElementRef<HTMLTextAreaElement>>('input');

  readonly profile = AGENT_PROFILE;
  readonly prompts = SUGGESTED_PROMPTS;
  readonly attackLabel = ATTACK_LABEL;
  readonly severityLabel = SEVERITY_LABEL;

  readonly draft = signal('');
  readonly leftOpen = signal(false);
  readonly rightOpen = signal(false);

  readonly nodes = computed(() => this.nodesRes().data ?? []);
  readonly active = this.agent.active;
  readonly messages = computed(() => this.active()?.messages ?? []);
  readonly saved = computed(() => this.agent.investigations().filter((i) => i.saved));
  readonly history = computed(() => this.agent.investigations().filter((i) => !i.saved));

  readonly contextNodeId = computed(() => this.active()?.context.nodeId ?? null);
  readonly contextNode = computed(() => this.nodes().find((n) => n.node_id === this.contextNodeId()) ?? null);
  readonly recentIncidents = computed(() =>
    (this.alertsRes().data ?? []).filter((a) => a.status !== 'resolved').slice(0, 5),
  );
  readonly relatedAlerts = computed(() => {
    const id = this.contextNodeId();
    const all = this.alertsRes().data ?? [];
    return (id ? all.filter((a) => a.node_id === id) : all.filter((a) => a.status === 'active')).slice(0, 5);
  });

  readonly connection = computed(() => {
    switch (this.agent.connection()) {
      case 'ready':
        return { tone: 'success', label: 'Connected', icon: 'plug' };
      case 'mock':
        return { tone: 'warning', label: 'Mock mode', icon: 'flask-conical' };
      default:
        return { tone: 'offline', label: 'Not connected', icon: 'unplug' };
    }
  });

  constructor() {
    // Deep links from Threats / Alerts / Nodes: /agent?node=…&alert=…&prediction=…
    const q = this.route.snapshot.queryParamMap;
    const node = q.get('node');
    const alert = q.get('alert');
    const prediction = q.get('prediction');
    if (node || alert || prediction) {
      const title = alert ? `Alert ${alert}` : prediction ? `Event on ${node ?? 'node'}` : `Investigation · ${node}`;
      this.agent.newInvestigation({ nodeId: node, alertId: alert, predictionId: prediction }, title);
      this.draft.set(
        alert
          ? `Investigate alert ${alert} on ${node}.`
          : prediction
            ? `Explain the latest threat prediction on ${node}.`
            : `Analyze the security status of ${node}.`,
      );
    }

    // Auto-scroll to the newest message.
    effect(() => {
      const count = this.messages().length;
      this.agent.pending();
      const el = untracked(() => this.thread())?.nativeElement;
      if (el && count > 0) queueMicrotask(() => (el.scrollTop = el.scrollHeight));
    });
  }

  newInvestigation(): void {
    this.agent.newInvestigation({ nodeId: this.contextNodeId() });
    this.leftOpen.set(false);
    this.focusInput();
  }

  openInvestigation(i: Investigation): void {
    this.agent.select(i.id);
    this.leftOpen.set(false);
  }

  investigateIncident(a: SecurityAlert): void {
    this.agent.newInvestigation({ nodeId: a.node_id, alertId: a.id }, `Alert ${a.id}`);
    this.draft.set(`Investigate the ${ATTACK_LABEL[a.attack_type]} alert ${a.id} on ${a.node_id}.`);
    this.leftOpen.set(false);
    this.focusInput();
  }

  setContextNode(id: string): void {
    const inv = this.active() ?? this.agent.newInvestigation();
    this.agent.setContext(inv.id, { nodeId: id || null });
  }

  usePrompt(p: string): void {
    this.draft.set(p);
    this.send();
  }

  send(): void {
    const text = this.draft().trim();
    if (!text || this.agent.pending()) return;
    this.agent.send(text);
    this.draft.set('');
  }

  onKey(e: KeyboardEvent): void {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      this.send();
    }
  }

  enableMock(): void {
    this.settings.agentMockMode.set(true);
  }

  copy(text: string): void {
    navigator.clipboard?.writeText(text).catch(() => undefined);
  }

  private focusInput(): void {
    queueMicrotask(() => this.inputEl()?.nativeElement.focus());
  }

  memPct(n: EthNode): number | null {
    const m = n.metrics;
    return m.memory_used_mb != null && m.memory_total_mb ? (m.memory_used_mb / m.memory_total_mb) * 100 : null;
  }
}
