/** Development environment (used by `ng serve`). */
export const environment = {
  production: false,
  apiBaseUrl: 'http://localhost:8080',
  apiPrefix: '/api/v1',
  pollingIntervalMs: 3000,
  requestTimeoutMs: 8000,
  agentEndpoint: null as string | null,
};
