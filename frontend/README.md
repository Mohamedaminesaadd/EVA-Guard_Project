# EVA-Guard — Frontend (Angular)

Datadog-inspired observability UI for **EVA-Guard**, the AI-powered Ethereum network security platform:
live node monitoring (Geth + Lighthouse beacon/validator), ML threat detection (NORMAL / ECLIPSE / RPC_FLOODING),
alert triage and an AI investigation workspace.

- Angular **22.3** · standalone components · signals + RxJS · zoneless
- SCSS design system (Inter + JetBrains Mono, self-hosted via `@fontsource`)
- Apache **ECharts 6** (tree-shaken, lazy-loaded) · **Lucide** icons (tree-shaken, no Angular wrapper)
- `marked` for agent markdown (output sanitized by Angular's DomSanitizer)

## Run

```bash
cd frontend
npm install
npm start                 # ng serve → http://localhost:4200
npm run build             # production build → dist/frontend
```

The UI calls the FastAPI backend at `http://localhost:8080/api/v1` (set in `src/environments/environment*.ts`,
overridable at runtime in **Settings → FastAPI connection**).

### FastAPI CORS (required)

The browser calls the API from another origin, so FastAPI must allow it:

```python
from fastapi.middleware.cors import CORSMiddleware

app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:4200"],
    allow_methods=["*"],
    allow_headers=["*"],
)
```

### Data source: Live vs Demo

| Mode | What you see |
|---|---|
| **Live** (default) | Only FastAPI data. A failed request shows an error state; the last real payload is kept and flagged *stale*. Nothing is ever replaced by fake values. |
| **Demo** | Deterministic simulation of `node-01…node-05` for UI review without a backend. A permanent banner and *Demo data* chips flag it on every page. Toggle in **Settings** or from the disconnected banner. |

## Architecture

```
src/app/
  core/
    api/normalizers.ts        ← the ONLY place that knows FastAPI payload shapes
    config/model-info.ts      ← static model facts + offline test metrics (95.56 % …)
    models/                   ← TypeScript interfaces (node, prediction, alert, dashboard, agent)
    services/
      api.service.ts          ← HttpClient wrapper: base URL, timeout, error mapping, probes
      polling.service.ts      ← shared polled resources (3 s, exhaustMap, shareReplay+refCount, pause when tab hidden)
      dashboard.service.ts    ← /dashboard/summary · distribution · timeline
      node.service.ts         ← /nodes, /nodes/{id} (+ merges /predictions/latest)
      prediction.service.ts   ← /predictions/latest, /predictions, /predictions/{id}
      alert.service.ts        ← /alerts + local-only acknowledge/resolve + new-alert notifications
      agent.service.ts        ← AI agent client (no endpoint yet → not-configured / mock mode)
      connection.service.ts   ← /health (connection indicator) + /status
      metrics-history.service.ts ← session time-series buffer built from /nodes polling
      mock-data.service.ts    ← DEMO MODE ONLY
  layout/                     ← shell: sidebar (collapsible, tooltips, mobile drawer), topbar
  features/                   ← dashboard, monitoring, threats, alerts, nodes (+ detail), agent,
                                ml-analytics, predictions, models, settings, not-found
  shared/
    charts/                   ← ECharts setup/theme, [appEchart] directive, option builders
    components/               ← metric-card, status-badge, chart-card, data-table, states
                                (loading/empty/error), time-range-selector, drawer, confidence, icon
    pipes/                    ← pct, bytes, ms, ago, dt …
```

**Live behaviour.** Every endpoint is one shared stream: however many components display it, there is exactly
**one request per endpoint per tick** (verified: 3 requests per endpoint in 9 s on the dashboard). Streams stop
when no page uses them, pause while the tab is hidden, and never stack a slow request with the next tick.
Charts are created once and updated in place with `setOption` (no destroy/recreate).

**Response shapes.** The Pydantic schemas were not available, so `normalizers.ts` accepts common variants:
bare arrays or `{items|data|nodes|predictions|alerts}` wrappers, nested `metrics`/`geth`/`beacon`/`validator`/`system`
groups, flat snake_case keys aligned with the Rust `Observation` schema (`peer_count`, `rpc_latency_ms`,
`cpu_usage_percent`…), `prediction`/`predicted_class`/`label`, confidences in 0–1 or 0–100, ISO (naive = UTC) or
epoch timestamps. If a field shows "—" while your API returns it, add its key to the alias lists there.

**Node status.** Used as-is when the backend sends `status`. Otherwise the UI derives it (badge *derived status*):
offline if Geth RPC is down · critical if the latest prediction is an attack · warning if CPU/memory ≥ 90 %,
sync distance > 32 or RPC latency > 1 s · healthy otherwise.

## Backend endpoints

Integrated (all `GET`, prefix `/api/v1`): `/health`, `/status`, `/nodes`, `/nodes/{node_id}`, `/predictions/latest`,
`/predictions` (sent with `?limit=1000`), `/predictions/{node_id}` (sent with `?limit=300`), `/alerts`,
`/dashboard/summary`, `/dashboard/distribution`, `/dashboard/timeline`.
**Settings → Endpoint diagnostics** calls each one and reports HTTP status and latency.

### Missing endpoints (UI ready, feature limited until they exist)

| Needed for | Suggested endpoint | Current UI behaviour |
|---|---|---|
| AI agent | `POST /api/v1/agent/chat` — request/response contract in `core/models/agent.model.ts` | *Not connected* state; optional, labelled mock mode |
| Alert triage | `PATCH /api/v1/alerts/{id}` (`status: acknowledged \| resolved`) | Acknowledge/Resolve stored locally, tagged *local* |
| Historical metrics | `GET /api/v1/nodes/{id}/metrics?from=&to=&step=` | Charts use a session buffer (≈2 h max); ranges beyond it are flagged |
| KPI trends | KPI history (or counts in `/dashboard/timeline`) | Sparklines/trends built from this session only |
| Model evaluation | `GET /api/v1/model/metrics` (incl. confusion matrix) | Offline metrics from `model-info.ts`; confusion matrix empty until filled |
| Model lifecycle | `POST /model`, `POST /model/reload`, `POST /model/train`, `GET /models` | Actions shown disabled |
| Threat investigation status | `PATCH /api/v1/predictions/{id}` | Stored locally, tagged *local only* |
| Server-side paging | `limit`, `offset`, `node_id`, `from`, `to` on `/predictions` | Client-side filter/sort/paging over the last 1000 |

## Notes

- Offline test metrics (accuracy 95.56 %, macro P/R/F1) are labelled *Offline test evaluation — not live accuracy*.
- Chart node colours come from a palette validated for colour-vision deficiency; status colours
  (green/amber/orange/red) are reserved for health and attack classes and always paired with a text label.
- Keyboard: `/` or `Ctrl+K` focuses global search (arrows + Enter), tables are sortable from the keyboard,
  drawers close with `Esc`.
