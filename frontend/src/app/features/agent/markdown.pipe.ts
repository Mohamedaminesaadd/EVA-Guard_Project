import { Pipe, PipeTransform } from '@angular/core';
import { marked } from 'marked';

marked.setOptions({ gfm: true, breaks: false });

/**
 * Markdown → HTML for agent messages. The output is bound with [innerHTML],
 * so Angular's DomSanitizer strips scripts, event handlers and unsafe URLs.
 */
@Pipe({ name: 'markdown' })
export class MarkdownPipe implements PipeTransform {
  transform(value: string | null | undefined): string {
    if (!value) return '';
    return marked.parse(value, { async: false }) as string;
  }
}
