import type { Run } from '@/types'
import { context, run, span, tool, usage } from './build'

export const CURRENT_SESSION = '4f2a91c3'

export function richRun(startedAt: number): Run {
  const sonnet = 'sonnet-4.6'
  return run({
    session: CURRENT_SESSION,
    index: 3,
    prompt: 'Fix the rounding in the interest calculation and add a test for it',
    started_at_ms: startedAt,
    model: sonnet,
    status: 'ok',
    signals: ['review'],
    spans: [
      span({
        id: 1, parent: 0, kind: 'run', name: 'run', start_ms: 0, end_ms: 94000,
        input: 'Fix the rounding in the interest calculation and add a test for it',
        output: "Rounding now uses banker's rounding through Decimal::round_dp_with_strategy. Added interest_rounds_half_to_even in tests/interest.rs; cargo test -p billing passes.",
        attrs: [['session', CURRENT_SESSION], ['branched from', '3c1e77b0 at checkpoint 4'], ['mode', 'accept edits'], ['sandbox', 'on, seatbelt'], ['memories', '3 recalled']],
      }),
      span({
        id: 2, parent: 1, kind: 'llm', name: sonnet, start_ms: 150, end_ms: 6200, stop: 'tool_use',
        usage: usage(sonnet, 1420, 15100, 1900, 410), context: context(4100, 8000, 5700),
        input: 'Fix the rounding in the interest calculation and add a test for it',
        thinking: 'Need to find where interest is computed before changing anything.',
        output: "I'll find where interest is computed.\n→ Grep(\"fn interest\", \"crates/billing\")",
      }),
      tool(3, 1, 'Grep', 'fn interest', 6250, 6400, { input: 'pattern: fn interest\npath: crates/billing', output: 'crates/billing/src/interest.rs:14: pub fn interest(principal: Decimal, rate: Decimal, days: u32) -> Decimal' }),
      tool(4, 1, 'Read', 'interest.rs', 6410, 6450, { input: 'crates/billing/src/interest.rs', output: '14  pub fn interest(principal: Decimal, rate: Decimal, days: u32) -> Decimal {\n15      let daily = rate / dec!(365);\n16      (principal * daily * Decimal::from(days)).round_dp(2)\n17  }', attrs: [['path', 'crates/billing/src/interest.rs']] }),
      span({
        id: 5, parent: 1, kind: 'llm', name: sonnet, start_ms: 6500, end_ms: 11800, stop: 'tool_use',
        usage: usage(sonnet, 1250, 18400, 0, 690), context: context(4100, 8000, 6930),
        input: '[tool result] 14  pub fn interest(...)',
        output: 'round_dp rounds half away from zero; the team memory says half-even.\n→ Edit(interest.rs)',
      }),
      tool(6, 1, 'Edit', 'interest.rs', 11850, 11900, {
        input: '-    (principal * daily * Decimal::from(days)).round_dp(2)\n+    (principal * daily * Decimal::from(days))\n+        .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven)',
        output: 'Edited crates/billing/src/interest.rs (lines 16-17)',
        flags: ['edited crates/billing/src/interest.rs'],
      }),
      span({ id: 7, parent: 6, kind: 'task', name: 'hook · cargo fmt', start_ms: 11900, end_ms: 12600, input: 'cargo fmt -p billing', output: 'ok' }),
      span({ id: 8, parent: 6, kind: 'task', name: 'affected tests', start_ms: 12600, end_ms: 19800, failed: true, input: 'cargo test -p billing', output: 'test interest::legacy_rounding ... FAILED\nexpected 10.01, got 10.00' }),
      span({
        id: 9, parent: 1, kind: 'llm', name: sonnet, start_ms: 19900, end_ms: 27000, stop: 'tool_use',
        usage: usage(sonnet, 1800, 19500, 0, 520), context: context(4100, 8000, 8580),
        input: '<tests> legacy_rounding FAILED', output: '→ Bash(cargo test -p billing)',
      }),
      tool(10, 1, 'Bash', 'cargo test -p billing', 27100, 41000, { failed: true, input: 'cargo test -p billing', output: 'DATABASE_URL=[redacted]\ntest interest::legacy_rounding ... FAILED\nexpected 10.01, got 10.00', attrs: [['exit code', '101'], ['redacted', 'DATABASE_URL']] }),
      span({
        id: 11, parent: 1, kind: 'llm', name: sonnet, start_ms: 41100, end_ms: 47300, stop: 'tool_use',
        usage: usage(sonnet, 1600, 21300, 0, 300), context: context(4100, 8000, 10180),
        input: '[tool result] FAILED', output: '→ Bash(cargo test -p billing)',
      }),
      tool(12, 1, 'Bash', 'cargo test -p billing', 47400, 47450, { failed: true, input: 'cargo test -p billing', output: 'Refused: the same call failed twice with the same output.', flags: ['repeated', 'refused: same failing call twice'] }),
      span({ id: 13, parent: 1, kind: 'event', name: 'escalated to advisor', start_ms: 47500, end_ms: 47500, output: 'Loop guard tripped; asking the advisor.' }),
      span({
        id: 14, parent: 1, kind: 'agent', name: 'advisor · stuck', start_ms: 60300, end_ms: 71000,
        input: 'Same test failed twice after the rounding change.',
        output: 'legacy_rounding encodes the old half-up rule. Update it to the half-even value (10.00) and rename it.',
        attrs: [['subagent', 'advisor'], ['model', 'opus-4.6']],
      }),
      span({
        id: 15, parent: 14, kind: 'llm', name: 'opus-4.6', start_ms: 60400, end_ms: 70900, stop: 'end_turn',
        usage: usage('opus-4.6', 24100, 0, 0, 880), context: context(3100, 0, 21000),
        input: '(advisor brief: the whole conversation so far)',
        thinking: 'The failing test asserts 10.01, which is half-up. The new rule is half-even, so 10.00 is right and the test is stale.',
        output: 'legacy_rounding encodes the old half-up rule. Update it to 10.00 and rename it.',
      }),
      span({
        id: 16, parent: 1, kind: 'llm', name: sonnet, start_ms: 71100, end_ms: 76000, stop: 'tool_use',
        usage: usage(sonnet, 2500, 22900, 0, 740), context: context(4100, 8000, 12680),
        input: '[advisor] update the old test', output: '→ Edit(tests/interest.rs)',
      }),
      tool(17, 1, 'Edit', 'tests/interest.rs', 76050, 76100, {
        input: '-fn legacy_rounding() { assert_eq!(..., dec!(10.01)) }\n+fn interest_rounds_half_to_even() { assert_eq!(..., dec!(10.00)) }',
        output: 'Edited crates/billing/tests/interest.rs',
        flags: ['edited crates/billing/tests/interest.rs'],
      }),
      span({ id: 18, parent: 1, kind: 'wait', name: 'permission · git commit', start_ms: 76200, end_ms: 84500, input: 'Bash: git commit -am "fix: half-even interest rounding"', output: 'Allowed once' }),
      tool(19, 1, 'Bash', 'git commit', 84500, 85100, { input: 'git commit -am "fix: half-even interest rounding"', output: '[main 8e1d2a0] fix: half-even interest rounding' }),
      span({
        id: 20, parent: 1, kind: 'llm', name: sonnet, start_ms: 85200, end_ms: 88900, stop: 'end_turn',
        usage: usage(sonnet, 1400, 25400, 0, 260), context: context(4100, 8000, 14080),
        input: '[tool result] [main 8e1d2a0]', output: "Rounding now uses banker's rounding through Decimal::round_dp_with_strategy...",
      }),
      span({
        id: 21, parent: 1, kind: 'eval', name: 'verification judge', start_ms: 89000, end_ms: 92600,
        usage: usage(sonnet, 9100, 0, 0, 210), input: 'criteria + diff', output: 'Criteria met: tests pass, rounding is half-even, a test covers it.',
      }),
      span({
        id: 22, parent: 1, kind: 'eval', name: 'auto-review', start_ms: 89000, end_ms: 93800, failed: true,
        usage: usage(sonnet, 11200, 0, 0, 330), input: 'diff + docs/', output: 'docs/billing.md still says "rounded half up" (line 41).',
      }),
    ],
  })
}
