import { ChangeDetectionStrategy, Component } from '@angular/core';
import { RouterLink } from '@angular/router';
import { EmptyStateComponent } from '../../shared/components/states/states';

@Component({
  selector: 'app-not-found-page',
  imports: [RouterLink, EmptyStateComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="card">
        <app-empty-state icon="circle-question-mark" title="Page not found" message="This route does not exist in EVA-Guard.">
          <a class="btn btn--primary" routerLink="/dashboard">Back to overview</a>
        </app-empty-state>
      </div>
    </div>
  `,
})
export class NotFoundPage {}
