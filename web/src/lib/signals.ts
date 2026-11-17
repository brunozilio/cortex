import { AlertCircle, Bot, CircleX, Layers, MessageSquare, Minimize2, Repeat, Ban, Undo2, type LucideIcon } from 'lucide-react'
import type { BadgeVariant } from '@/components/ui/badge'
import type { Finding, Signal } from '@/types'

export interface SignalStyle {
  variant: BadgeVariant
  icon: LucideIcon
}

export const SIGNALS: Record<Signal, SignalStyle> = {
  tool_error: { variant: 'danger', icon: AlertCircle },
  eval_failed: { variant: 'danger', icon: CircleX },
  review: { variant: 'warning', icon: MessageSquare },
  subagent: { variant: 'secondary', icon: Bot },
  waste: { variant: 'warning', icon: Layers },
  repeated: { variant: 'danger', icon: Repeat },
  fell_back: { variant: 'warning', icon: Undo2 },
  compacted: { variant: 'secondary', icon: Minimize2 },
  refused: { variant: 'danger', icon: Ban },
}

export const SIGNAL_ORDER: Signal[] = ['tool_error', 'eval_failed', 'review', 'repeated', 'refused', 'subagent', 'waste', 'fell_back', 'compacted']

export function checkVariant(finding: Finding): BadgeVariant {
  if (finding.check === 'review') return finding.severe ? 'danger' : 'warning'
  if (finding.check === 'repeated') return 'secondary'
  return finding.severe ? 'danger' : 'warning'
}
