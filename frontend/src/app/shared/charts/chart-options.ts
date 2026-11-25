import type { EChartsCoreOption } from 'echarts/core';

/**
 * Option builders. Conventions (see dataviz rules):
 * - one y-axis per chart (never dual axis),
 * - 2 px lines, no symbols, recessive grid,
 * - legend only when ≥ 2 series, crosshair tooltip on time series,
 * - 2 px surface gap between stacked / adjacent fills,
 * - animations off for live-updating charts.
 */
export interface TimeSeries {
  name: string;
  color: string;
  data: [number, number | null][];
  dashed?: boolean;
}

export type ValueFormatter = (v: number) => string;

const SURFACE = '#FFFFFF';

function axisTimeLabel(value: number): string {
  const d = new Date(value);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}`;
}

export function timeSeriesOption(cfg: {
  series: TimeSeries[];
  format: ValueFormatter;
  yMin?: number;
  yMax?: number;
  area?: boolean;
  step?: boolean;
  xMin?: number;
  xMax?: number;
  splitNumber?: number;
  interval?: number;
}): EChartsCoreOption {
  const multi = cfg.series.length > 1;
  return {
    animation: false,
    grid: { left: 4, right: 12, top: multi ? 30 : 12, bottom: 4, outerBoundsMode: 'same', outerBoundsContain: 'axisLabel' },
    legend: multi ? { show: true, type: 'scroll' } : { show: false },
    tooltip: {
      trigger: 'axis',
      valueFormatter: (v: unknown) => (typeof v === 'number' ? cfg.format(v) : '—'),
      axisPointer: { type: 'line' },
    },
    xAxis: {
      type: 'time',
      min: cfg.xMin,
      max: cfg.xMax,
      axisLabel: { formatter: axisTimeLabel },
    },
    yAxis: {
      type: 'value',
      min: cfg.yMin,
      max: cfg.yMax,
      axisLabel: { formatter: (v: number) => cfg.format(v) },
      splitNumber: cfg.splitNumber ?? 3,
      interval: cfg.interval ?? (cfg.yMin === 0 && cfg.yMax === 100 ? 25 : cfg.yMin === 0 && cfg.yMax === 1 ? 0.5 : undefined),
    },
    series: cfg.series.map((s) => ({
      name: s.name,
      type: 'line',
      // Sparse series (e.g. a live buffer that just started) get visible points.
      showSymbol: s.data.filter(([, v]) => v !== null).length < 12,
      symbolSize: 6,
      step: cfg.step ? 'end' : undefined,
      connectNulls: false,
      sampling: 'lttb',
      lineStyle: { width: 2, color: s.color, type: s.dashed ? 'dashed' : 'solid' },
      itemStyle: { color: s.color },
      areaStyle: cfg.area && !multi ? { color: s.color, opacity: 0.08 } : undefined,
      emphasis: { focus: 'series' },
      data: s.data,
    })),
  };
}

export function stackedTimeBarOption(cfg: {
  series: { name: string; color: string; data: [number, number][] }[];
  format: ValueFormatter;
}): EChartsCoreOption {
  return {
    animation: false,
    grid: { left: 4, right: 12, top: 30, bottom: 4, outerBoundsMode: 'same', outerBoundsContain: 'axisLabel' },
    legend: { show: true },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
      valueFormatter: (v: unknown) => (typeof v === 'number' ? cfg.format(v) : '—'),
    },
    xAxis: { type: 'time', axisLabel: { formatter: axisTimeLabel } },
    yAxis: { type: 'value', minInterval: 1, splitNumber: 3 },
    series: cfg.series.map((s, i) => ({
      name: s.name,
      type: 'bar',
      stack: 'total',
      barMaxWidth: 18,
      itemStyle: {
        color: s.color,
        borderColor: SURFACE,
        borderWidth: 1,
        borderRadius: i === cfg.series.length - 1 ? [3, 3, 0, 0] : 0,
      },
      emphasis: { focus: 'series' },
      data: s.data,
    })),
  };
}

export function donutOption(cfg: {
  data: { name: string; value: number; color: string }[];
  centerValue: string;
  centerLabel: string;
  format?: ValueFormatter;
}): EChartsCoreOption {
  const total = cfg.data.reduce((a, d) => a + d.value, 0);
  return {
    animation: false,
    tooltip: {
      trigger: 'item',
      formatter: (p: { name: string; value: number; marker: string }) =>
        `${p.marker} ${p.name}<br/><b>${(cfg.format ?? String)(p.value)}</b> · ${total ? ((p.value / total) * 100).toFixed(1) : '0.0'}%`,
    },
    legend: { show: true, bottom: 0, left: 'center', top: 'auto' },
    title: {
      text: cfg.centerValue,
      subtext: cfg.centerLabel,
      left: 'center',
      top: '33%',
      textStyle: { fontSize: 22, fontWeight: 600, color: '#1D2939', fontFamily: 'JetBrains Mono, monospace' },
      subtextStyle: { fontSize: 11, color: '#667085' },
      itemGap: 4,
    },
    series: [
      {
        type: 'pie',
        radius: ['58%', '78%'],
        center: ['50%', '44%'],
        padAngle: 1.5,
        avoidLabelOverlap: true,
        label: { show: false },
        labelLine: { show: false },
        itemStyle: { borderRadius: 3, borderColor: SURFACE, borderWidth: 1 },
        emphasis: { scale: true, scaleSize: 4 },
        data: cfg.data.map((d) => ({ name: d.name, value: d.value, itemStyle: { color: d.color } })),
      },
    ],
  };
}

export function sparklineOption(values: (number | null)[], color: string): EChartsCoreOption {
  return {
    animation: false,
    grid: { left: 0, right: 0, top: 2, bottom: 2 },
    tooltip: { show: false },
    xAxis: { type: 'category', show: false, boundaryGap: false, data: values.map((_, i) => i) },
    yAxis: { type: 'value', show: false, scale: true },
    series: [
      {
        type: 'line',
        data: values,
        showSymbol: false,
        connectNulls: false,
        lineStyle: { width: 1.5, color },
        areaStyle: { color, opacity: 0.08 },
        silent: true,
      },
    ],
  };
}

export function barOption(cfg: {
  categories: string[];
  series: { name: string; color: string; data: (number | null)[] }[];
  format: ValueFormatter;
  horizontal?: boolean;
  stacked?: boolean;
}): EChartsCoreOption {
  const multi = cfg.series.length > 1;
  const cat = { type: 'category', data: cfg.categories };
  const val = { type: 'value', splitNumber: 3, axisLabel: { formatter: (v: number) => cfg.format(v) } };
  return {
    animation: false,
    grid: { left: 4, right: 16, top: multi ? 30 : 10, bottom: 4, outerBoundsMode: 'same', outerBoundsContain: 'axisLabel' },
    legend: multi ? { show: true } : { show: false },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
      valueFormatter: (v: unknown) => (typeof v === 'number' ? cfg.format(v) : '—'),
    },
    xAxis: cfg.horizontal ? val : cat,
    yAxis: cfg.horizontal ? cat : val,
    series: cfg.series.map((s) => ({
      name: s.name,
      type: 'bar',
      stack: cfg.stacked ? 'total' : undefined,
      barMaxWidth: 22,
      barGap: '10%',
      itemStyle: {
        color: s.color,
        borderColor: SURFACE,
        borderWidth: 1,
        borderRadius: cfg.horizontal ? [0, 3, 3, 0] : [3, 3, 0, 0],
      },
      data: s.data,
    })),
  };
}

export function heatmapOption(cfg: { matrix: number[][]; labels: string[] }): EChartsCoreOption {
  const data: [number, number, number][] = [];
  let max = 0;
  cfg.matrix.forEach((row, y) =>
    row.forEach((v, x) => {
      data.push([x, y, v]);
      max = Math.max(max, v);
    }),
  );
  return {
    animation: false,
    grid: { left: 4, right: 8, top: 8, bottom: 30, outerBoundsMode: 'same', outerBoundsContain: 'axisLabel' },
    tooltip: {
      formatter: (p: { value: [number, number, number] }) =>
        `True <b>${cfg.labels[p.value[1]]}</b><br/>Predicted <b>${cfg.labels[p.value[0]]}</b><br/>${p.value[2]} samples`,
    },
    xAxis: { type: 'category', data: cfg.labels, name: 'Predicted', nameLocation: 'middle', nameGap: 22 },
    yAxis: { type: 'category', data: cfg.labels, inverse: true },
    visualMap: { show: false, min: 0, max, inRange: { color: ['#F4F3FF', '#635BFF'] } },
    series: [
      {
        type: 'heatmap',
        data: data.map((d) => ({ value: d, label: { color: d[2] > max * 0.55 ? '#FFFFFF' : '#1D2939' } })),
        label: { show: true, fontFamily: 'JetBrains Mono, monospace' },
        itemStyle: { borderColor: SURFACE, borderWidth: 2, borderRadius: 3 },
      },
    ],
  };
}
