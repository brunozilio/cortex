# Angular portals

## Build and verify

Use the package manager whose lock file is there (`npm ci`, `yarn
--frozen-lockfile`, `pnpm i --frozen-lockfile`) and the scripts in
`package.json` before the bare CLI.

| Step | Command |
|---|---|
| Fast loop | `npx ng test --watch=false --include src/app/refunds/**/*.spec.ts` |
| Unit tests | `npx ng test --watch=false --browsers=ChromeHeadless` |
| Lint | `npx ng lint` |
| Build | `npx ng build --configuration production` |
| End to end | `npx cypress run` (see `dev-flow:test-automation`) |

The production build catches template and type errors the dev server
lets through; it is part of the verification.

## Code

- Follow the application's structure and its Angular version's idioms:
  standalone components and signals where the code already uses them,
  modules and RxJS where it does not. Do not migrate styles inside a
  story.
- Components show and collect; HTTP calls and rules live in services.
  A component never calls `HttpClient` directly.
- Types for every API payload, generated from the contract when the
  project does that; no `any`.
- Subscriptions end with the component (`async` pipe,
  `takeUntilDestroyed`); no manual subscription left open.
- Forms are reactive forms with their validation messages; every input
  has a label, every action works from the keyboard.
- User-facing text goes through the project's translation mechanism when
  it has one.
- Add `data-cy` attributes to the elements a Cypress scenario touches.
- Environment values come from the existing environment or runtime
  configuration; no URL or key in a component.

## Tests

- Unit tests beside the source (`*.spec.ts`) with the runner the project
  has (Karma and Jasmine, or Jest): services with `HttpTestingController`,
  components through what they render, not their private fields.
- Acceptance scenarios are Cypress specs (`dev-flow:test-automation`).
