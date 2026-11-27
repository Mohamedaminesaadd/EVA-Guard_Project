import { Pipe, PipeTransform } from '@angular/core';
import {
  fmtBytes,
  fmtCompact,
  fmtDateTime,
  fmtDuration,
  fmtMb,
  fmtMs,
  fmtNumber,
  fmtPct,
  fmtPercentValue,
  fmtTime,
  timeAgo,
} from '../../core/utils/format';

/** 0..1 → "96.4%" */
@Pipe({ name: 'pct' })
export class PctPipe implements PipeTransform {
  transform(v: number | null | undefined, digits = 1): string {
    return fmtPct(v, digits);
  }
}

/** 0..100 → "42.0%" */
@Pipe({ name: 'percent100' })
export class Percent100Pipe implements PipeTransform {
  transform(v: number | null | undefined, digits = 1): string {
    return fmtPercentValue(v, digits);
  }
}

@Pipe({ name: 'num' })
export class NumPipe implements PipeTransform {
  transform(v: number | null | undefined, digits = 0): string {
    return fmtNumber(v, digits);
  }
}

@Pipe({ name: 'compact' })
export class CompactPipe implements PipeTransform {
  transform(v: number | null | undefined): string {
    return fmtCompact(v);
  }
}

@Pipe({ name: 'bytes' })
export class BytesPipe implements PipeTransform {
  transform(v: number | null | undefined, perSec = false): string {
    return fmtBytes(v, perSec);
  }
}

@Pipe({ name: 'mb' })
export class MbPipe implements PipeTransform {
  transform(v: number | null | undefined): string {
    return fmtMb(v);
  }
}

@Pipe({ name: 'ms' })
export class MsPipe implements PipeTransform {
  transform(v: number | null | undefined): string {
    return fmtMs(v);
  }
}

@Pipe({ name: 'duration' })
export class DurationPipe implements PipeTransform {
  transform(v: number | null | undefined): string {
    return fmtDuration(v);
  }
}

/** Relative time; pass the clock signal value as argument so it refreshes: `ts | ago: clock.now()` */
@Pipe({ name: 'ago' })
export class AgoPipe implements PipeTransform {
  transform(v: string | number | null | undefined, now?: number): string {
    return timeAgo(v, now ?? Date.now());
  }
}

/** Local "yyyy-MM-dd HH:mm:ss" (or time only with `false`). */
@Pipe({ name: 'dt' })
export class DateTimePipe implements PipeTransform {
  transform(v: string | number | null | undefined, withDate = true): string {
    return withDate ? fmtDateTime(v) : fmtTime(v);
  }
}

export const FORMAT_PIPES = [
  PctPipe,
  Percent100Pipe,
  NumPipe,
  CompactPipe,
  BytesPipe,
  MbPipe,
  MsPipe,
  DurationPipe,
  AgoPipe,
  DateTimePipe,
] as const;
