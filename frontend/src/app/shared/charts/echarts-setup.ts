import { BarChart, HeatmapChart, LineChart, PieChart } from 'echarts/charts';
import {
  DataZoomComponent,
  GridComponent,
  LegendComponent,
  MarkLineComponent,
  TitleComponent,
  TooltipComponent,
  VisualMapComponent,
} from 'echarts/components';
import * as echarts from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';

/** Tree-shaken ECharts build + EVA-Guard theme (Inter, recessive grid, Datadog-like tooltips). */
echarts.use([
  LineChart,
  BarChart,
  PieChart,
  HeatmapChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
  MarkLineComponent,
  TitleComponent,
  VisualMapComponent,
  DataZoomComponent,
  CanvasRenderer,
]);

const TEXT = '#1D2939';
const TEXT_2 = '#667085';
const GRID = '#F2F4F7';
const AXIS = '#EAECF0';
const FONT = "Inter, -apple-system, 'Segoe UI', sans-serif";

echarts.registerTheme('evaguard', {
  color: ['#2E6FDB', '#0B9A8D', '#7A5AF8', '#946200', '#D9508F'],
  backgroundColor: 'transparent',
  textStyle: { fontFamily: FONT, color: TEXT_2, fontSize: 11 },
  grid: { left: 8, right: 12, top: 28, bottom: 8, outerBoundsMode: 'same', outerBoundsContain: 'axisLabel' },
  legend: {
    top: 0,
    left: 0,
    icon: 'roundRect',
    itemWidth: 10,
    itemHeight: 3,
    itemGap: 14,
    textStyle: { color: TEXT, fontSize: 11 },
  },
  tooltip: {
    backgroundColor: '#FFFFFF',
    borderColor: '#EAECF0',
    borderWidth: 1,
    padding: [8, 10],
    textStyle: { color: TEXT, fontSize: 12, fontFamily: FONT },
    extraCssText: 'box-shadow: 0 12px 24px -6px rgba(16,24,40,.14); border-radius: 6px;',
    axisPointer: { lineStyle: { color: '#98A2B3', type: 'dashed', width: 1 } },
  },
  timeAxis: {
    axisLine: { show: true, lineStyle: { color: AXIS } },
    axisTick: { show: false },
    axisLabel: { color: TEXT_2, fontSize: 10, hideOverlap: true },
    splitLine: { show: false },
  },
  categoryAxis: {
    axisLine: { show: true, lineStyle: { color: AXIS } },
    axisTick: { show: false },
    axisLabel: { color: TEXT_2, fontSize: 10, hideOverlap: true },
    splitLine: { show: false },
  },
  valueAxis: {
    axisLine: { show: false },
    axisTick: { show: false },
    axisLabel: { color: TEXT_2, fontSize: 10 },
    splitLine: { lineStyle: { color: GRID } },
  },
  line: { symbol: 'none', lineStyle: { width: 2 }, emphasis: { lineStyle: { width: 2 } } },
  bar: { itemStyle: { borderRadius: [2, 2, 0, 0] } },
});

export { echarts };
