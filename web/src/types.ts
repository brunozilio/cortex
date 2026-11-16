export type SpanKind = 'run' | 'llm' | 'tool' | 'agent' | 'task' | 'eval' | 'wait' | 'event';
export interface Usage { model: string; input: number; output: number; cache_read: number; cache_write: number; cost_usd: number }
export interface ContextSplit { system: number; tools: number; conversation: number }
export interface Span { id: number; parent: number; kind: SpanKind; name: string; start_ms: number; end_ms: number; failed: boolean; input: string; output: string; thinking: string; usage: Usage | null; context: ContextSplit | null; stop: string | null; attrs: [string, string][]; flags: string[]; request?: number[]; reply?: Message; system_id?: string; tools_id?: string }
export type Signal = 'tool_error' | 'eval_failed' | 'review' | 'subagent' | 'waste' | 'repeated' | 'fell_back' | 'compacted' | 'refused';
export interface Run { session: string; index: number; prompt: string; started_at_ms: number; duration_ms: number; waiting_ms: number; model: string; status: 'ok' | 'failed' | 'cancelled' | 'running'; cost_usd: number; tokens: number; input: number; cache_read: number; waste: number; signals: Signal[]; spans: Span[]; messages?: Message[]; systems?: Record<string, string>; tools?: Record<string, ToolSpec[]> }
export interface SessionRuns { id: string; runs: Run[]; insights?: Insights }
export interface Day { day: number; models: { model: string; cost_usd: number; tokens: number; runs: number }[] }
export interface ModelStat { model: string; calls: number; p50_ms: number; p95_ms: number; cache_ratio: number; cost_usd: number }
export interface ToolStat { name: string; calls: number; failed: number; p95_ms: number; asked: number }
export interface Checks { verified_passed: number; verified_failed: number; review_findings: number; repeated: number; refused: number; asked: number }
export interface Totals { runs: number; cost_usd: number; tokens: number; waiting_ms: number; p95_run_ms: number; tool_error_rate: number; cache_ratio: number }
export interface Finding { session: string; run: number; span: number; check: 'judge' | 'review' | 'repeated' | 'refused'; text: string; at_ms: number; severe: boolean }
export interface Insights { days: Day[]; models: ModelStat[]; tools: ToolStat[]; time: { kind: SpanKind; ms: number }[]; checks: Checks; totals: Totals; findings: Finding[] }
export interface Report { project: string; current: string; generated_at_ms: number; sessions: SessionRuns[]; insights: Insights }
export type Block =
  | { Text: string }
  | { Reminder: string }
  | { Image: { media_type: string; data: string } }
  | { Thinking: { text: string; signature: string | null } }
  | { ToolUse: { id: string; name: string; input: unknown } }
  | { ToolResult: { id: string; content: string; is_error: boolean } }
  | { ToolResultRich: { id: string; content: string; images: { media_type: string; data: string }[]; is_error: boolean } }
  | { Native: { origin: string; item: unknown } };
export interface Message { role: 'User' | 'Assistant'; blocks: Block[] }
export interface ToolSpec { name: string; description?: string; schema?: unknown }
