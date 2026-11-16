import { AlertCircle, Bot, CircleX, Layers, MessageSquare, Minimize2, Repeat, Ban, Undo2, type LucideIcon } from 'lucide-react'
import type { BadgeVariant } from '@/components/ui/badge'
import type { Finding, Run, Signal } from '@/types'

export interface SignalStyle {
  label: string
  variant: BadgeVariant
  icon: LucideIcon
}

export const SIGNALS: Record<Signal, SignalStyle> = {
  tool_error: { label: 'Tool error', variant: 'danger', icon: AlertCircle },
  eval_failed: { label: 'Eval failed', variant: 'danger', icon: CircleX },
  review: { label: 'Review', variant: 'warning', icon: MessageSquare },
  subagent: { label: 'Subagent', variant: 'secondary', icon: Bot },
  waste: { label: 'Waste', variant: 'warning', icon: Layers },
  repeated: { label: 'Repeated', variant: 'danger', icon: Repeat },
  fell_back: { label: 'Fell back', variant: 'warning', icon: Undo2 },
  compacted: { label: 'Compacted', variant: 'secondary', icon: Minimize2 },
  refused: { label: 'Refused', variant: 'danger', icon: Ban },
}

export const SIGNAL_ORDER: Signal[] = ['tool_error', 'eval_failed', 'review', 'repeated', 'refused', 'subagent', 'waste', 'fell_back', 'compacted']

export const STATUS_LABELS: Record<Run['status'], string> = {
  ok: 'Succeeded',
  failed: 'Failed',
  cancelled: 'Cancelled',
  running: 'Running',
}

export const CHECK_LABELS: Record<Finding['check'], string> = {
  judge: 'judge',
  review: 'auto-review',
  repeated: 'loop guard',
  refused: 'refusal',
}

export function checkVariant(finding: Finding): BadgeVariant {
  if (finding.check === 'review') return finding.severe ? 'danger' : 'warning'
  if (finding.check === 'repeated') return 'secondary'
  return finding.severe ? 'danger' : 'warning'
}
