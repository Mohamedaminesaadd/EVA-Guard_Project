/**
 * Production environment.
 * The API base URL can also be overridden at runtime from Settings (stored in localStorage).
 */
export const environment = {
  production: true,
  /** FastAPI backend base URL (no trailing slash). */
  apiBaseUrl: 'http://localhost:8080',
  /** Prefix of every REST endpoint. */
  apiPrefix: '/api/v1',
  /** Live polling interval in milliseconds. */
  pollingIntervalMs: 3000,
  /** HTTP timeout for a single request in milliseconds. */
  requestTimeoutMs: 8000,
  /**
   * AI agent endpoint. Intentionally `null`: no agent API exists yet.
   * When the LangChain/CrewAI agent is exposed by FastAPI, set the full URL here
   * (e.g. 'http://localhost:8080/api/v1/agent/chat') and adapt AgentService.toRequest().
   */
  agentEndpoint: null as string | null,
};
