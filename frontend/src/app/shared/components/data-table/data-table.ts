import { NgTemplateOutlet } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  Directive,
  TemplateRef,
  computed,
  contentChildren,
  effect,
  inject,
  input,
  output,
  signal,
} from '@angular/core';
import { IconComponent } from '../icon/icon';

export interface TableColumn<T> {
  key: string;
  label: string;
  sortable?: boolean;
  align?: 'left' | 'right' | 'center';
  mono?: boolean;
  width?: string;
  /** Plain-text cell value (used when no custom cell template is given). */
  value?: (row: T) => string | number | null | undefined;
  /** Value used for sorting (defaults to `value`). */
  sortValue?: (row: T) => string | number | null | undefined;
  hideBelow?: 'md' | 'lg';
}

export type SortDir = 'asc' | 'desc';

/** Custom cell template: `<ng-template appCell="status" let-row>…</ng-template>` */
@Directive({ selector: 'ng-template[appCell]' })
export class CellDefDirective {
  readonly name = input.required<string>({ alias: 'appCell' });
  readonly template = inject<TemplateRef<{ $implicit: unknown }>>(TemplateRef);
}

/** Sortable, paginated table with keyboard-accessible clickable rows. */
@Component({
  selector: 'app-data-table',
  imports: [NgTemplateOutlet, IconComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="table-wrap" [style.max-height]="maxHeight() || null">
      <table class="table" [class.table--compact]="compact()">
        <thead>
          <tr>
            @for (c of columns(); track c.key) {
              <th
                [class.is-sortable]="c.sortable"
                [class.is-num]="c.align === 'right'"
                [class.hide-md]="c.hideBelow === 'md'"
                [class.hide-lg]="c.hideBelow === 'lg'"
                [style.width]="c.width || null"
                [attr.aria-sort]="sortKey() === c.key ? (sortDir() === 'asc' ? 'ascending' : 'descending') : null"
                [attr.tabindex]="c.sortable ? 0 : null"
                (click)="c.sortable && toggleSort(c.key)"
                (keydown.enter)="c.sortable && toggleSort(c.key)"
              >
                <span class="th-sort">
                  {{ c.label }}
                  @if (c.sortable) {
                    <app-icon
                      [name]="sortKey() === c.key ? (sortDir() === 'asc' ? 'arrow-up' : 'arrow-down') : 'chevrons-up-down'"
                      [size]="11"
                      [class.sort-idle]="sortKey() !== c.key"
                    />
                  }
                </span>
              </th>
            }
          </tr>
        </thead>
        <tbody>
          @for (row of pageRows(); track trackKey()(row)) {
            <tr
              [class.is-clickable]="clickable()"
              [class.is-selected]="selectedKey() !== null && trackKey()(row) === selectedKey()"
              [attr.tabindex]="clickable() ? 0 : null"
              (click)="clickable() && rowClick.emit(row)"
              (keydown.enter)="clickable() && rowClick.emit(row)"
            >
              @for (c of columns(); track c.key) {
                <td
                  [class.is-num]="c.align === 'right'"
                  [class.mono]="c.mono"
                  [class.hide-md]="c.hideBelow === 'md'"
                  [class.hide-lg]="c.hideBelow === 'lg'"
                >
                  @if (cellTemplate(c.key); as tpl) {
                    <ng-container *ngTemplateOutlet="tpl; context: { $implicit: row }" />
                  } @else {
                    {{ display(c, row) }}
                  }
                </td>
              }
            </tr>
          }
        </tbody>
      </table>
    </div>
    @if (pageSize() > 0 && sorted().length > 0) {
      <div class="pager">
        <span class="pager__info mono">
          {{ rangeStart() + 1 }}–{{ rangeEnd() }} of {{ sorted().length }}
        </span>
        <label class="pager__size">
          Rows
          <select class="select select--sm" [value]="size()" (change)="setSize($any($event.target).value)">
            @for (o of pageSizeOptions(); track o) {
              <option [value]="o">{{ o }}</option>
            }
          </select>
        </label>
        <div class="pager__nav">
          <button class="btn btn--sm btn--icon" type="button" [disabled]="page() === 0" (click)="page.set(page() - 1)" aria-label="Previous page">
            <app-icon name="chevron-left" [size]="14" />
          </button>
          <span class="mono pager__page">{{ page() + 1 }} / {{ pageCount() }}</span>
          <button class="btn btn--sm btn--icon" type="button" [disabled]="page() >= pageCount() - 1" (click)="page.set(page() + 1)" aria-label="Next page">
            <app-icon name="chevron-right" [size]="14" />
          </button>
        </div>
      </div>
    }
  `,
  styles: `
    :host { display: block; min-width: 0; }
    .sort-idle { opacity: .35; }
    .pager { display: flex; align-items: center; gap: 16px; padding: 8px 12px; border-top: 1px solid var(--eg-border); font-size: 12px; color: var(--eg-text-2); flex-wrap: wrap; }
    .pager__info { margin-right: auto; }
    .pager__size { display: inline-flex; align-items: center; gap: 6px; }
    .select--sm { height: 26px; font-size: 12px; padding-left: 8px; }
    .pager__nav { display: inline-flex; align-items: center; gap: 6px; }
    .pager__page { min-width: 52px; text-align: center; }
    @media (max-width: 900px) { .hide-md { display: none; } }
    @media (max-width: 1200px) { .hide-lg { display: none; } }
  `,
})
export class DataTableComponent<T> {
  readonly columns = input.required<TableColumn<T>[]>();
  readonly rows = input.required<T[]>();
  readonly trackKey = input<(row: T) => string>((row: T) => String((row as { id?: unknown }).id ?? JSON.stringify(row)));
  readonly pageSize = input(0);
  readonly pageSizeOptions = input<number[]>([25, 50, 100]);
  readonly clickable = input(false);
  readonly compact = input(false);
  readonly selectedKey = input<string | null>(null);
  readonly initialSort = input<{ key: string; dir: SortDir } | null>(null);
  readonly maxHeight = input<string>('');
  readonly rowClick = output<T>();

  private readonly cellDefs = contentChildren(CellDefDirective);

  readonly sortKey = signal<string | null>(null);
  readonly sortDir = signal<SortDir>('desc');
  readonly page = signal(0);
  readonly size = signal(25);

  constructor() {
    effect(() => {
      const s = this.initialSort();
      if (s && this.sortKey() === null) {
        this.sortKey.set(s.key);
        this.sortDir.set(s.dir);
      }
    });
    effect(() => {
      const p = this.pageSize();
      if (p > 0) this.size.set(p);
    });
    // Clamp page when data shrinks (filters).
    effect(() => {
      const count = this.pageCount();
      if (this.page() > count - 1) this.page.set(Math.max(0, count - 1));
    });
  }

  readonly sorted = computed(() => {
    const rows = this.rows();
    const key = this.sortKey();
    if (!key) return rows;
    const col = this.columns().find((c) => c.key === key);
    if (!col) return rows;
    const get = col.sortValue ?? col.value ?? ((r: T) => (r as Record<string, unknown>)[key] as string | number | null);
    const dir = this.sortDir() === 'asc' ? 1 : -1;
    return [...rows].sort((a, b) => {
      const va = get(a);
      const vb = get(b);
      if (va === vb) return 0;
      if (va === null || va === undefined) return 1;
      if (vb === null || vb === undefined) return -1;
      if (typeof va === 'number' && typeof vb === 'number') return (va - vb) * dir;
      return String(va).localeCompare(String(vb), undefined, { numeric: true }) * dir;
    });
  });

  readonly pageCount = computed(() => (this.pageSize() > 0 ? Math.max(1, Math.ceil(this.sorted().length / this.size())) : 1));
  readonly rangeStart = computed(() => this.page() * this.size());
  readonly rangeEnd = computed(() => Math.min(this.sorted().length, this.rangeStart() + this.size()));
  readonly pageRows = computed(() =>
    this.pageSize() > 0 ? this.sorted().slice(this.rangeStart(), this.rangeEnd()) : this.sorted(),
  );

  toggleSort(key: string): void {
    if (this.sortKey() === key) {
      this.sortDir.set(this.sortDir() === 'asc' ? 'desc' : 'asc');
    } else {
      this.sortKey.set(key);
      this.sortDir.set('desc');
    }
    this.page.set(0);
  }

  setSize(v: string): void {
    this.size.set(Number(v));
    this.page.set(0);
  }

  cellTemplate(key: string): TemplateRef<{ $implicit: unknown }> | null {
    return this.cellDefs().find((d) => d.name() === key)?.template ?? null;
  }

  display(c: TableColumn<T>, row: T): string {
    const v = c.value ? c.value(row) : (row as Record<string, unknown>)[c.key];
    return v === null || v === undefined || v === '' ? '—' : String(v);
  }
}
