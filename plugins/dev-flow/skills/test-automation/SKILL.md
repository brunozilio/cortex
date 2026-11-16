---
name: test-automation
description: How the development flow automates and runs a story's test scenarios - Cucumber with JUnit for APIs, Cypress for the Angular portals - with every scenario tied to a Zephyr test case. Use when writing the automated tests of a story in the implement stage, or when running and recording them in the qa stage.
---

# Test automation in the flow

A scenario is written once, in Gherkin, in the spec. It becomes a Zephyr
test case in `plan` (`dev-flow:zephyr`), an automated test in
`implement`, and a recorded execution in `qa`. API scenarios run in
Cucumber on JUnit; web scenarios run in Cypress against the Angular
portal.

Read the repository before writing a test: where features, steps and
specs live, how the suite is started, which helpers exist. Follow what is
there; the layouts below are only the usual ones.

## API scenarios: Cucumber on JUnit

- Feature files under `src/test/resources/features/`, one per
  capability. Copy the scenario from the spec and tag it with the story
  and its Zephyr key:

  ```gherkin
  @PAY-123
  Feature: Refunds

    @TestCaseKey=PAY-T45
    Scenario: Refund a captured payment in full
      Given a captured payment of 100.00
      When the merchant refunds it in full
      Then the payment is refunded
      And the merchant balance is reduced by 100.00
  ```

- Step definitions call the API over HTTP as a client would (the
  project's REST client: REST Assured, WebTestClient...). Reuse an
  existing step before writing a new one; a step states one thing and
  holds no scenario-specific data.
- Each scenario creates the data it needs and leaves nothing behind; it
  never depends on another scenario's order.
- Run only the story while building, the whole suite before the PR:

  ```sh
  mvn -q verify -Dcucumber.filter.tags="@PAY-123"
  ./gradlew test -Dcucumber.filter.tags="@PAY-123"
  ```

  The run writes `cucumber.json` (and JUnit XML) under `target/` or
  `build/`: that is what goes to Zephyr.

## Web scenarios: Cypress on the Angular portal

- Specs under `cypress/e2e/`, one per flow. The Zephyr key opens the
  test title:

  ```ts
  describe("PAY-123 refunds", () => {
    it("PAY-T46 shows the refund in the payment history", () => {
      cy.login("merchant");
      cy.visit("/payments/123");
      cy.get("[data-cy=refund]").click();
      cy.get("[data-cy=history]").should("contain", "Refunded");
    });
  });
  ```

- Select by `data-cy` attributes; add one to the Angular template when
  it is missing rather than selecting by CSS class or text that changes.
- No fixed waits (`cy.wait(3000)`): wait on the request
  (`cy.intercept` + `cy.wait("@alias")`) or on what the page shows.
- Test through the portal what the user does; rules already covered by
  API scenarios are not repeated click by click.
- Run:

  ```sh
  npx cypress run --spec "cypress/e2e/refunds.cy.ts"
  npx cypress run --env grepTags=@PAY-123
  ```

  The second form needs `@cypress/grep`; use what the project has.

## Stage qa: running against the QA environment

1. Find how the project points a run at QA (a Maven profile, a
   `BASE_URL`, a Cypress `--config baseUrl=` or `--env`); ask the person
   for the URL and credentials when the repository does not say. Never
   write credentials to a file.
2. Confirm QA runs the merged build (a version endpoint, the pipeline's
   last deploy) before testing; results from an older build are worth
   nothing.
3. Run the story's API and web scenarios, then the regression suite the
   team names in `config.json` (`qa.regression`) when there is one.
4. Open the cycle and record every execution with its real result and
   the build it ran against (`dev-flow:zephyr`), or upload the Cucumber
   and JUnit results.
5. Write `.flow/<ISSUE>/qa.md`: environment and build, commands, a table
   of scenario, Zephyr key and result, and for each failure what
   happened against what was expected, with the screenshot or response.

A failing scenario is investigated before it is called flaky: rerun it
alone once, read the trace. A real failure sends the flow back to
`implement`; a scenario that cannot run (environment down, data missing)
is recorded `Blocked`, never `Pass`, and the stage does not end.
