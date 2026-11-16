# Java services

## Build and verify

| Build | Fast loop | Full verification |
|---|---|---|
| Maven | `./mvnw -q test -Dtest=RefundServiceTest` | `./mvnw -q verify` |
| Gradle | `./gradlew test --tests '*RefundServiceTest'` | `./gradlew check` |

`verify`/`check` runs unit tests, integration tests and the Cucumber
suite, plus whatever the build wires in (formatting, Checkstyle,
coverage thresholds). Fix a formatting failure with the build's own
goal (`spotless:apply`, `spotlessApply`), not by hand.

## Code

- Keep the layers the service has (controller or handler, service,
  repository or client). Business rules stay out of controllers and out
  of persistence classes.
- Constructor injection; no field injection, no static state.
- Validate input at the edge (Bean Validation on request types) and map
  domain errors to HTTP in one place (the existing exception handler).
- A changed API contract changes its OpenAPI file or annotations in the
  same commit; a removed or renamed field is a breaking change and needs
  the spec to say how consumers move.
- Database changes go through the migration tool in use (Flyway,
  Liquibase): a new migration file, never an edited old one, and safe to
  run while the previous version of the service is still up.
- AWS access goes through the SDK clients the service already
  configures; no credentials in code or properties, names and ARNs come
  from configuration.
- Log with the existing logger and its structured fields; never log
  tokens, card data or personal data.

## Tests

- Unit tests with JUnit 5 and the mocking and assertion libraries
  already on the classpath; one behaviour per test, named for it.
- Integration tests use what the project has (Spring Boot test slices,
  Testcontainers, LocalStack); do not mock the database in a test that
  claims to test persistence.
- Acceptance scenarios are Cucumber features (`dev-flow:test-automation`).
