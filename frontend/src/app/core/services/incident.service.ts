import { Injectable, inject } from '@angular/core';
import { Observable } from 'rxjs';
import { normalizeIncident, normalizeIncidents, normalizeInvestigationReport, normalizeInvestigationReports } from '../api/normalizers';
import { Resource } from '../models/common.model';
import { Incident, InvestigationReport } from '../models/incident.model';
import { API_ENDPOINTS, ApiService } from './api.service';
import { PollingService } from './polling.service';

const LIST_LIMIT = 200;

/** Incidents and AI investigations only come from the live backend: demo mode shows nothing rather than invented reports. */
function liveOnly(what: string): never {
  throw new Error(`${what} are only available from the live FastAPI backend (Settings → Data source → Live).`);
}

/**
 * GET /api/v1/incidents[/{id}] and /api/v1/investigations[/{incident_id}].
 * Incidents are created by the ML backend for eligible attacks; the reports are
 * produced by the LangGraph Hypothesis → Confirmation workflow.
 */
@Injectable({ providedIn: 'root' })
export class IncidentService {
  private readonly api = inject(ApiService);
  private readonly polling = inject(PollingService);
  private readonly incidentCache = new Map<string, Observable<Resource<Incident>>>();
  private readonly reportCache = new Map<string, Observable<Resource<InvestigationReport>>>();

  readonly incidents$ = this.polling.poll<Incident[]>({
    live: () => this.api.get(API_ENDPOINTS.incidents, { limit: LIST_LIMIT }),
    normalize: normalizeIncidents,
    demo: () => [],
    intervalMultiplier: 2,
  });

  readonly investigations$ = this.polling.poll<InvestigationReport[]>({
    live: () => this.api.get(API_ENDPOINTS.investigations, { limit: LIST_LIMIT }),
    normalize: normalizeInvestigationReports,
    demo: () => [],
    intervalMultiplier: 2,
  });

  incident$(id: string): Observable<Resource<Incident>> {
    let s = this.incidentCache.get(id);
    if (!s) {
      s = this.polling.poll<Incident>({
        live: () => this.api.get(API_ENDPOINTS.incident(id)),
        normalize: (raw) => {
          const i = normalizeIncident(raw);
          if (!i) throw new Error(`Unexpected payload for incident ${id}`);
          return i;
        },
        demo: () => liveOnly('Incidents'),
        intervalMultiplier: 2,
      });
      this.incidentCache.set(id, s);
    }
    return s;
  }

  investigation$(incidentId: string): Observable<Resource<InvestigationReport>> {
    let s = this.reportCache.get(incidentId);
    if (!s) {
      s = this.polling.poll<InvestigationReport>({
        live: () => this.api.get(API_ENDPOINTS.investigation(incidentId)),
        normalize: (raw) => {
          const r = normalizeInvestigationReport(raw);
          if (!r) throw new Error(`Unexpected payload for investigation ${incidentId}`);
          return r;
        },
        demo: () => liveOnly('Investigation reports'),
        intervalMultiplier: 2,
      });
      this.reportCache.set(incidentId, s);
    }
    return s;
  }
}
