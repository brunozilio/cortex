---
name: zephyr
description: How the development flow keeps test scenarios in Zephyr Scale Cloud through its REST API - create test cases with Gherkin scripts, link them to the Jira issue, open a test cycle, record executions, upload Cucumber or JUnit results. Use when a flow stage registers scenarios or records test results, for API or web tests alike.
---

# Zephyr Scale in the flow

Every scenario, API or web, has a test case in Zephyr Scale. The flow
creates the cases in `plan`, tags the automated tests with their keys in
`implement`, and records the executions in `qa`.

## Access

Zephyr Scale Cloud has a REST API, version 2, with a bearer token a
person creates in Jira (their profile, "Zephyr Scale API Access Tokens").
The token is read from the environment variable `config.json` names
(`zephyr.tokenEnv`, default `ZEPHYR_TOKEN`). Never print it, never write
it to a file. If it is empty, say so and stop.

```sh
zephyr() {
  curl -sS --fail-with-body -H "Authorization: Bearer $ZEPHYR_TOKEN" \
    -H "Content-Type: application/json" "$@"
}
BASE=https://api.zephyrscale.smartbear.com/v2
```

Sites hosted in the EU use `https://eu.api.zephyrscale.smartbear.com/v2`
(`zephyr.base` in `config.json` overrides the default). On a 4xx, read
the body: it names the wrong field. The reference is
`https://support.smartbear.com/zephyr-scale-cloud/api-docs/`; check it
before guessing a field.

The gate refuses writes to this API before the flow reached `plan`.

## Test cases (stage plan)

Look before creating, so a rerun does not duplicate:

```sh
zephyr "$BASE/testcases?projectKey=PAY&maxResults=100"
```

Create one case per scenario. Put the payload in a file under
`.flow/<ISSUE>/` so the person sees what is sent:

```sh
zephyr -X POST "$BASE/testcases" -d @.flow/PAY-123/case-1.json
```

```json
{
  "projectKey": "PAY",
  "name": "Refund a captured payment in full",
  "objective": "Acceptance criterion 1 of PAY-123",
  "labels": ["api", "PAY-123"]
}
```

The answer carries the key (`PAY-T45`). Then its Gherkin script and its
link to the story (the link takes Jira's numeric issue id, which
`dev-flow:jira`'s `getJiraIssue` returns as `id`):

```sh
zephyr -X POST "$BASE/testcases/PAY-T45/testscript" \
  -d '{"type": "bdd", "text": "Given a captured payment\nWhen ..."}'
zephyr -X POST "$BASE/testcases/PAY-T45/links/issues" -d '{"issueId": 10234}'
```

Label each case `api` or `web`, and record the keys in the state file's
`tests`.

## Carrying the key in the automated test

- Cucumber: tag the scenario `@TestCaseKey=PAY-T45`.
- Cypress: start the test title with the key, `it("PAY-T46 shows the
  refund in the history", ...)`.

## Cycle and executions (stage qa)

```sh
zephyr -X POST "$BASE/testcycles" \
  -d '{"projectKey": "PAY", "name": "PAY-123 QA 2026-11-16"}'
```

Record the cycle key (`PAY-R12`) in the state file's `cycle`. Then one
execution per case, with the result the run really had:

```sh
zephyr -X POST "$BASE/testexecutions" -d '{
  "projectKey": "PAY", "testCaseKey": "PAY-T45", "testCycleKey": "PAY-R12",
  "statusName": "Pass", "environmentName": "QA",
  "comment": "cucumber, build 4812"
}'
```

`statusName` is one of the project's statuses (`Pass`, `Fail`,
`Blocked`, `Not Executed` by default). A test that did not run is never
recorded as `Pass`.

A whole Cucumber or JUnit run can be uploaded instead, as a zip of the
result files; Zephyr matches the cases by the keys above:

```sh
curl -sS --fail-with-body -H "Authorization: Bearer $ZEPHYR_TOKEN" \
  -F "file=@target/cucumber-reports.zip" \
  "$BASE/automations/executions/cucumber?projectKey=PAY&autoCreateTestCases=false"
```

(`/automations/executions/junit` for JUnit XML.) Keep
`autoCreateTestCases=false`: cases are created in `plan`, on purpose.
