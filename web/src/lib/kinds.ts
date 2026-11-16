import { Bot, CircleDot, Diamond, Hourglass, Play, ShieldCheck, Sparkles, Wrench, Zap, type LucideIcon } from 'lucide-react'
import type { SpanKind } from '@/types'

export interface KindStyle {
  label: string
  icon: LucideIcon
  bar: string
  chip: string
  text: string
  swatch: string
}

export const KINDS: Record<SpanKind, KindStyle> = {
  run: { label: 'Run', icon: Play, bar: 'bg-k-run', chip: 'bg-k-run/15 text-k-run', text: 'text-k-run', swatch: 'bg-k-run' },
  agent: { label: 'Agent', icon: Bot, bar: 'bg-k-agent', chip: 'bg-k-agent/15 text-k-agent', text: 'text-k-agent', swatch: 'bg-k-agent' },
  llm: { label: 'LLM', icon: Sparkles, bar: 'bg-k-llm', chip: 'bg-k-llm/15 text-k-llm', text: 'text-k-llm', swatch: 'bg-k-llm' },
  tool: { label: 'Tool', icon: Wrench, bar: 'bg-k-tool', chip: 'bg-k-tool/15 text-k-tool', text: 'text-k-tool', swatch: 'bg-k-tool' },
  task: { label: 'Task', icon: Zap, bar: 'bg-k-task', chip: 'bg-k-task/15 text-k-task', text: 'text-k-task', swatch: 'bg-k-task' },
  eval: { label: 'Evaluation', icon: ShieldCheck, bar: 'bg-k-eval', chip: 'bg-k-eval/15 text-k-eval', text: 'text-k-eval', swatch: 'bg-k-eval' },
  wait: { label: 'Waiting on you', icon: Hourglass, bar: 'hatch', chip: 'bg-muted text-muted-foreground', text: 'text-muted-foreground', swatch: 'hatch' },
  event: { label: 'Event', icon: Diamond, bar: 'bg-k-event', chip: 'bg-k-event/15 text-k-event', text: 'text-k-event', swatch: 'bg-k-event' },
}

export const LEGEND_KINDS: SpanKind[] = ['agent', 'llm', 'tool', 'task', 'eval', 'wait', 'event']

export const TIME_LABELS: Partial<Record<SpanKind, string>> = {
  llm: 'Model calls',
  tool: 'Tools',
  wait: 'Waiting on you',
  agent: 'Subagents',
  task: 'Hooks and tests',
  eval: 'Evaluations',
}

export const MODEL_COLORS = ['hsl(var(--chart-1))', 'hsl(var(--chart-2))', 'hsl(var(--chart-3))', 'hsl(var(--chart-4))']

export const FallbackIcon = CircleDot
