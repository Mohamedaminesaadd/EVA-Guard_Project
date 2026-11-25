import { Routes } from '@angular/router';
import { MainLayoutComponent } from './layout/main-layout/main-layout';

const t = (title: string) => `${title} · EVA-Guard`;

export const routes: Routes = [
  {
    path: '',
    component: MainLayoutComponent,
    children: [
      { path: '', pathMatch: 'full', redirectTo: 'dashboard' },
      {
        path: 'dashboard',
        title: t('Overview'),
        data: { section: 'Overview', title: 'Overview' },
        loadComponent: () => import('./features/dashboard/dashboard.page').then((m) => m.DashboardPage),
      },
      {
        path: 'monitoring',
        title: t('Network Monitoring'),
        data: { section: 'Overview', title: 'Network Monitoring' },
        loadComponent: () => import('./features/monitoring/monitoring.page').then((m) => m.MonitoringPage),
      },
      {
        path: 'threats',
        title: t('Threat Detection'),
        data: { section: 'Security', title: 'Threat Detection' },
        loadComponent: () => import('./features/threats/threats.page').then((m) => m.ThreatsPage),
      },
      {
        path: 'alerts',
        title: t('Security Alerts'),
        data: { section: 'Security', title: 'Security Alerts' },
        loadComponent: () => import('./features/alerts/alerts.page').then((m) => m.AlertsPage),
      },
      {
        path: 'nodes',
        data: { section: 'Security', title: 'Nodes' },
        children: [
          {
            path: '',
            title: t('Nodes'),
            loadComponent: () => import('./features/nodes/nodes.page').then((m) => m.NodesPage),
          },
          {
            path: ':id',
            title: t('Node details'),
            data: { title: 'Node', crumbParam: 'id' },
            loadComponent: () => import('./features/nodes/node-detail.page').then((m) => m.NodeDetailPage),
          },
        ],
      },
      {
        path: 'incidents',
        data: { section: 'Security', title: 'Incidents' },
        children: [
          {
            path: '',
            title: t('Incidents'),
            loadComponent: () => import('./features/incidents/incidents.page').then((m) => m.IncidentsPage),
          },
          {
            path: ':incidentId',
            title: t('Incident details'),
            data: { title: 'Incident', crumbParam: 'incidentId' },
            loadComponent: () => import('./features/incidents/incident-detail.page').then((m) => m.IncidentDetailPage),
          },
        ],
      },
      {
        path: 'investigations',
        title: t('AI Investigations'),
        data: { section: 'Intelligence', title: 'AI Investigations' },
        loadComponent: () => import('./features/investigations/investigations.page').then((m) => m.InvestigationsPage),
      },
      {
        path: 'agent',
        title: t('AI Agent'),
        data: { section: 'Intelligence', title: 'AI Agent' },
        loadComponent: () => import('./features/agent/agent.page').then((m) => m.AgentPage),
      },
      {
        path: 'ml-analytics',
        title: t('ML Analytics'),
        data: { section: 'Intelligence', title: 'ML Analytics' },
        loadComponent: () => import('./features/ml-analytics/ml-analytics.page').then((m) => m.MlAnalyticsPage),
      },
      {
        path: 'predictions',
        title: t('Prediction History'),
        data: { section: 'Intelligence', title: 'Prediction History' },
        loadComponent: () => import('./features/predictions/predictions.page').then((m) => m.PredictionsPage),
      },
      {
        path: 'models',
        title: t('Model Management'),
        data: { section: 'System', title: 'Model Management' },
        loadComponent: () => import('./features/models/model-management.page').then((m) => m.ModelManagementPage),
      },
      {
        path: 'settings',
        title: t('Settings'),
        data: { section: 'System', title: 'Settings' },
        loadComponent: () => import('./features/settings/settings.page').then((m) => m.SettingsPage),
      },
      {
        path: '**',
        title: t('Not found'),
        data: { title: 'Not found' },
        loadComponent: () => import('./features/not-found/not-found.page').then((m) => m.NotFoundPage),
      },
    ],
  },
];
