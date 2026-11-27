export interface NavItem {
  label: string;
  icon: string;
  route: string;
  /** Live counter shown next to the item. */
  badge?: 'alerts';
  keywords?: string;
}

export interface NavSection {
  title: string;
  items: NavItem[];
}

export const NAV_SECTIONS: NavSection[] = [
  {
    title: 'Overview',
    items: [
      { label: 'Overview', icon: 'layout-dashboard', route: '/dashboard', keywords: 'dashboard home kpi' },
      { label: 'Network Monitoring', icon: 'monitor-activity', route: '/monitoring', keywords: 'metrics infrastructure cpu memory geth beacon validator' },
    ],
  },
  {
    title: 'Security',
    items: [
      { label: 'Threat Detection', icon: 'radar', route: '/threats', keywords: 'eclipse rpc flooding attack events' },
      { label: 'Security Alerts', icon: 'bell-ring', route: '/alerts', badge: 'alerts', keywords: 'incidents acknowledge resolve' },
      { label: 'Nodes', icon: 'server', route: '/nodes', keywords: 'ethereum geth lighthouse' },
      { label: 'Incidents', icon: 'shield-exclamation-point', route: '/incidents', keywords: 'incident attack verdict confirmed suspected' },
    ],
  },
  {
    title: 'Intelligence',
    items: [
      { label: 'AI Investigations', icon: 'scan-search', route: '/investigations', keywords: 'langgraph hypothesis confirmation agent rag report' },
      { label: 'AI Agent', icon: 'bot', route: '/agent', keywords: 'assistant investigation chat llm' },
      { label: 'ML Analytics', icon: 'brain-circuit', route: '/ml-analytics', keywords: 'model accuracy confusion features' },
      { label: 'Prediction History', icon: 'history', route: '/predictions', keywords: 'predictions csv export probabilities' },
    ],
  },
  {
    title: 'System',
    items: [
      { label: 'Model Management', icon: 'boxes', route: '/models', keywords: 'model version registry' },
      { label: 'Settings', icon: 'settings', route: '/settings', keywords: 'api url polling demo mode' },
    ],
  },
];

export const ALL_NAV_ITEMS: NavItem[] = NAV_SECTIONS.flatMap((s) => s.items);
