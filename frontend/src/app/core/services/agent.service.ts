import { HttpClient } from '@angular/common/http';
import { Injectable, computed, inject, signal } from '@angular/core';
import { Observable, delay, map, of, throwError, timeout } from 'rxjs';
import { environment } from '../../../environments/environment';
import { normalizeAttack, normalizeConfidence, severityForAttack } from '../api/normalizers';
import {
  AgentMessage,
  AgentRequest,
  AgentResponse,
  Investigation,
  InvestigationContext,
  ThreatAnalysis,
} from '../models/agent.model';
import { Severity } from '../models/common.model';
import { readJson, removeKey, writeJson } from '../utils/storage';
import { toApiError } from './api.service';
import { SettingsService } from './settings.service';

export type AgentConnection = 'ready' | 'mock' | 'not-configured';

const STORE_KEY = 'eva-guard.investigations.v1';

export const AGENT_PROFILE = {
  name: 'EVA Security Agent',
  description: 'AI-powered Ethereum security investigation assistant.',
};

export const SUGGESTED_PROMPTS = [
  'Analyze the current Ethereum network security status.',
  'Investigate possible Eclipse attacks.',
  'Explain why node-03 has abnormal RPC activity.',
  'Compare the latest ML predictions.',
  'Investigate RPC flooding behavior.',
  'Summarize the latest security alerts.',
];

/**
 * Front-end side of the future AI agent (LangChain / CrewAI + RAG).
 * No agent endpoint exists yet: `environment.agentEndpoint` is null, so the
 * service reports `not-configured`. An optional, clearly-labelled mock mode
 * returns illustrative responses to design and test the UI.
 */
@Injectable({ providedIn: 'root' })
export class AgentService {
  private readonly http = inject(HttpClient);
  private readonly settings = inject(SettingsService);

  readonly endpoint = environment.agentEndpoint;
  readonly connection = computed<AgentConnection>(() =>
    this.endpoint ? 'ready' : this.settings.agentMockMode() ? 'mock' : 'not-configured',
  );

  private readonly _investigations = signal<Investigation[]>(readJson<Investigation[]>(STORE_KEY, []));
  readonly investigations = this._investigations.asReadonly();
  readonly activeId = signal<string | null>(this._investigations()[0]?.id ?? null);
  readonly active = computed(() => this._investigations().find((i) => i.id === this.activeId()) ?? null);
  readonly pending = signal(false);

  newInvestigation(context: Partial<InvestigationContext> = {}, title = 'New investigation'): Investigation {
    const inv: Investigation = {
      id: crypto.randomUUID(),
      title,
      createdAt: Date.now(),
      updatedAt: Date.now(),
      saved: false,
      context: { nodeId: null, alertId: null, predictionId: null, ...context },
      messages: [],
    };
    this._investigations.update((l) => [inv, ...l].slice(0, 50));
    this.activeId.set(inv.id);
    this.persist();
    return inv;
  }

  select(id: string): void {
    this.activeId.set(id);
  }

  toggleSaved(id: string): void {
    this.patch(id, (i) => ({ ...i, saved: !i.saved }));
  }

  remove(id: string): void {
    this._investigations.update((l) => l.filter((i) => i.id !== id));
    if (this.activeId() === id) this.activeId.set(this._investigations()[0]?.id ?? null);
    this.persist();
  }

  setContext(id: string, ctx: Partial<InvestigationContext>): void {
    this.patch(id, (i) => ({ ...i, context: { ...i.context, ...ctx } }));
  }

  clearAll(): void {
    this._investigations.set([]);
    this.activeId.set(null);
    removeKey(STORE_KEY);
  }

  /** Sends a user message in the active investigation and appends the agent reply. */
  send(text: string): void {
    const content = text.trim();
    if (!content || this.pending()) return;
    const inv = this.active() ?? this.newInvestigation();
    const userMsg: AgentMessage = { id: crypto.randomUUID(), role: 'user', content, createdAt: Date.now() };
    this.patch(inv.id, (i) => ({
      ...i,
      title: i.messages.length === 0 ? content.slice(0, 60) : i.title,
      messages: [...i.messages, userMsg],
    }));

    this.pending.set(true);
    const current = this._investigations().find((i) => i.id === inv.id)!;
    this.request(current, content).subscribe({
      next: (reply) => this.append(inv.id, reply),
      error: (e: unknown) => {
        const err = toApiError(e);
        this.append(inv.id, {
          id: crypto.randomUUID(),
          role: 'system',
          content:
            err.status === -3
              ? err.message
              : `**Agent request failed.** ${err.message}`,
          createdAt: Date.now(),
          error: true,
        });
      },
    });
  }

  private request(inv: Investigation, text: string): Observable<AgentMessage> {
    const conn = this.connection();
    if (conn === 'not-configured') {
      return throwError(() => ({
        status: -3,
        network: false,
        message:
          '**No AI agent endpoint is configured.** The EVA Security Agent backend has not been integrated yet. ' +
          'Set `agentEndpoint` in `src/environments/environment*.ts` once it exists, or enable **mock mode** to preview the interface.',
      }));
    }
    if (conn === 'mock') return of(this.mockReply(text, inv.context)).pipe(delay(900 + Math.random() * 700));

    const body: AgentRequest = {
      conversation_id: inv.id,
      message: text,
      history: inv.messages.filter((m) => m.role !== 'system').map((m) => ({ role: m.role, content: m.content })),
      context: { node_id: inv.context.nodeId, alert_id: inv.context.alertId, prediction_id: inv.context.predictionId },
    };
    return this.http.post<AgentResponse>(this.endpoint!, body).pipe(
      timeout(120_000),
      map((res) => ({
        id: crypto.randomUUID(),
        role: 'agent' as const,
        content: res.message,
        createdAt: Date.now(),
        analysis: res.analysis
          ? {
              attackType: normalizeAttack(res.analysis.attack_type),
              nodeId: res.analysis.node_id,
              confidence: normalizeConfidence(res.analysis.confidence),
              severity: (['healthy', 'low', 'medium', 'high', 'critical'].includes(res.analysis.severity?.toLowerCase())
                ? res.analysis.severity.toLowerCase()
                : severityForAttack(normalizeAttack(res.analysis.attack_type))) as Severity,
              evidence: res.analysis.evidence ?? [],
              recommendations: res.analysis.recommendations ?? [],
            }
          : undefined,
      })),
    );
  }

  private append(id: string, msg: AgentMessage): void {
    this.pending.set(false);
    this.patch(id, (i) => ({ ...i, messages: [...i.messages, msg] }));
  }

  private patch(id: string, fn: (i: Investigation) => Investigation): void {
    this._investigations.update((l) => l.map((i) => (i.id === id ? { ...fn(i), updatedAt: Date.now() } : i)));
    this.persist();
  }

  private persist(): void {
    writeJson(STORE_KEY, this._investigations());
  }

  // -------------------------------------------------------------------------
  // MOCK MODE — illustrative UI examples, not actual analysis results.
  // -------------------------------------------------------------------------
  private mockReply(text: string, ctx: InvestigationContext): AgentMessage {
    const q = text.toLowerCase();
    const base = { id: crypto.randomUUID(), role: 'agent' as const, createdAt: Date.now(), mock: true };
    const node = ctx.nodeId ?? (q.match(/node-\d+/)?.[0] ?? 'node-03');

    if (q.includes('eclipse')) {
      const analysis: ThreatAnalysis = {
        attackType: 'ECLIPSE',
        nodeId: node,
        confidence: 0.912,
        severity: 'critical',
        evidence: ['Sharp drop of connected peers', 'Peer churn above baseline', 'Beacon sync distance increasing'],
        recommendations: ['List current peers with `admin.peers` and check IP diversity', 'Compare head slot with a trusted node', 'Add static/trusted peers if the peer set stays narrow'],
      };
      return {
        ...base,
        content:
          `### Eclipse attack investigation\n\nAn eclipse attack isolates a node by monopolising its peer connections. ` +
          `I would look at **peer count**, **peer churn** and **head lag** together.\n\n` +
          '```bash\n# Inspect peers on the suspected node (Geth console)\ngeth attach --exec "admin.peers.map(p => p.network.remoteAddress)"\n```',
        analysis,
      };
    }
    if (q.includes('rpc') || q.includes('flood')) {
      const analysis: ThreatAnalysis = {
        attackType: 'RPC_FLOODING',
        nodeId: node,
        confidence: 0.964,
        severity: 'high',
        evidence: ['Abnormal RPC request rate.', 'Increased RPC latency.', 'Suspicious network activity.'],
        recommendations: ['Inspect RPC request sources.', 'Compare peer activity.', 'Review recent node logs.'],
      };
      return {
        ...base,
        content:
          `### RPC activity on \`${node}\`\n\nRPC flooding saturates the JSON-RPC interface with requests, raising CPU and latency. ` +
          `The structured result below shows the format the agent will return.\n\n` +
          '```bash\n# Top RPC clients by connection count\nss -tn state established "( sport = :8545 )" | awk \'{print $4}\' | cut -d: -f1 | sort | uniq -c | sort -rn | head\n```',
        analysis,
      };
    }
    if (q.includes('compare') || q.includes('prediction')) {
      return {
        ...base,
        content:
          `### Comparing ML predictions\n\nOnce connected, the agent will read \`GET /api/v1/predictions/latest\` and compare classes and confidences per node.\n\n` +
          `| Node | Prediction | Confidence |\n|---|---|---|\n| node-01 | NORMAL | 97.2% |\n| node-03 | RPC_FLOODING | 96.4% |\n| node-05 | ECLIPSE | 91.2% |\n\n` +
          `> Values above are an illustrative layout, not live predictions.`,
      };
    }
    if (q.includes('alert') || q.includes('summar')) {
      return {
        ...base,
        content:
          `### Alert summary (format preview)\n\n- **Critical** — Eclipse suspected on one node, peer set collapsed\n- **High** — RPC flooding, request rate far above baseline\n- **Resolved** — earlier flooding episode\n\nThe real agent will ground this list in \`GET /api/v1/alerts\` and the knowledge base.`,
      };
    }
    return {
      ...base,
      content:
        `### Network security status (format preview)\n\nThe EVA Security Agent will combine:\n\n1. **Live node metrics** — Geth, Lighthouse beacon and validator\n2. **ML predictions** — Logistic Regression over 69 features\n3. **Knowledge base** — RAG over Ethereum attack documentation\n\nAsk about a specific node or attack type to see a structured investigation card.`,
    };
  }
}
