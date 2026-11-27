import { Injectable, signal } from '@angular/core';

/** 1-second clock signal used by the topbar clock and relative-time labels. */
@Injectable({ providedIn: 'root' })
export class ClockService {
  readonly now = signal(Date.now());

  constructor() {
    setInterval(() => this.now.set(Date.now()), 1000);
  }
}
