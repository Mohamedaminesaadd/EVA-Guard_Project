import { Injectable, computed, signal } from '@angular/core';
import { Tone } from '../utils/labels';

export interface AppNotification {
  id: string;
  title: string;
  message: string;
  tone: Tone;
  icon: string;
  createdAt: number;
  link?: string;
  read: boolean;
}

export interface Toast extends AppNotification {
  timeoutMs: number;
}

/** In-app notification centre (bell) + transient toasts. */
@Injectable({ providedIn: 'root' })
export class NotificationService {
  private readonly _items = signal<AppNotification[]>([]);
  private readonly _toasts = signal<Toast[]>([]);

  readonly items = this._items.asReadonly();
  readonly toasts = this._toasts.asReadonly();
  readonly unreadCount = computed(() => this._items().filter((n) => !n.read).length);

  push(n: Omit<AppNotification, 'id' | 'createdAt' | 'read'>, toast = true): void {
    const item: AppNotification = { ...n, id: crypto.randomUUID(), createdAt: Date.now(), read: false };
    this._items.update((list) => [item, ...list].slice(0, 50));
    if (toast) {
      const t: Toast = { ...item, timeoutMs: 6000 };
      this._toasts.update((list) => [t, ...list].slice(0, 4));
      setTimeout(() => this.dismissToast(t.id), t.timeoutMs);
    }
  }

  dismissToast(id: string): void {
    this._toasts.update((list) => list.filter((t) => t.id !== id));
  }

  markAllRead(): void {
    this._items.update((list) => list.map((n) => ({ ...n, read: true })));
  }

  clear(): void {
    this._items.set([]);
  }
}
