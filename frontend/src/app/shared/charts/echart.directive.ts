import { DestroyRef, Directive, ElementRef, afterNextRender, effect, inject, input, output, signal } from '@angular/core';
import type { EChartsCoreOption } from 'echarts/core';
import { echarts } from './echarts-setup';

/**
 * Binds an ECharts instance to its host element.
 * The instance is created once and updated in place with `setOption` (replaceMerge
 * on series), so live polling never destroys/recreates charts.
 */
@Directive({
  selector: '[appEchart]',
  host: { style: 'display:block; width:100%; min-width:0' },
})
export class EchartDirective {
  readonly options = input.required<EChartsCoreOption | null>({ alias: 'appEchart' });
  readonly replaceMerge = input<string[]>(['series', 'xAxis', 'yAxis', 'legend']);
  readonly chartClick = output<unknown>();

  private readonly el = inject<ElementRef<HTMLElement>>(ElementRef);
  private readonly ready = signal(false);
  private chart: echarts.ECharts | null = null;

  constructor() {
    const destroyRef = inject(DestroyRef);
    let ro: ResizeObserver | null = null;

    afterNextRender(() => {
      // Initialise only once the host has a size (it may be projected while hidden).
      // Work is deferred to the next frame to avoid "ResizeObserver loop" errors.
      let frame = 0;
      ro = new ResizeObserver(() => {
        cancelAnimationFrame(frame);
        frame = requestAnimationFrame(() => {
          const el = this.el.nativeElement;
          if (!el.isConnected || !el.clientWidth || !el.clientHeight) return;
          if (!this.chart) {
            this.chart = echarts.init(el, 'evaguard', { renderer: 'canvas' });
            this.chart.on('click', (p) => this.chartClick.emit(p));
            this.ready.set(true);
          } else {
            this.chart.resize({ animation: { duration: 0 } });
          }
        });
      });
      ro.observe(this.el.nativeElement);
    });

    effect(() => {
      const opts = this.options();
      if (!this.ready() || !this.chart || !opts) return;
      this.chart.setOption(opts, { replaceMerge: this.replaceMerge(), lazyUpdate: true });
    });

    destroyRef.onDestroy(() => {
      ro?.disconnect();
      this.chart?.dispose();
      this.chart = null;
    });
  }
}
