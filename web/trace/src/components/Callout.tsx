import type { ReactNode } from 'react'
import type { LucideIcon } from 'lucide-react'
import { cn } from '@/lib/utils'

type Tone = 'danger' | 'warning' | 'success' | 'neutral'

const TONES: Record<Tone, { box: string; icon: string }> = {
  danger: { box: 'border-danger/30 bg-danger/5', icon: 'text-danger' },
  warning: { box: 'border-warning/30 bg-warning/5', icon: 'text-warning' },
  success: { box: 'border-success/30 bg-success/5', icon: 'text-success' },
  neutral: { box: '', icon: 'text-muted-foreground' },
}

interface CalloutProps {
  tone: Tone
  icon: LucideIcon
  title: ReactNode
  children?: ReactNode
  className?: string
}

export function Callout({ tone, icon: Icon, title, children, className }: CalloutProps) {
  const style = TONES[tone]
  return (
    <div className={cn('grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 rounded-lg border p-4 text-sm', style.box, className)}>
      <Icon className={cn('mt-0.5 size-4', style.icon)} />
      <div className="font-medium">{title}</div>
      {children ? (
        <>
          <div />
          <div className="grid min-w-0 gap-3 break-words text-muted-foreground">{children}</div>
        </>
      ) : null}
    </div>
  )
}
