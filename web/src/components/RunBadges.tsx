import { Ban, CircleCheck, CircleX, Loader } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { SIGNALS, SIGNAL_ORDER } from '@/lib/signals'
import { cn } from '@/lib/utils'
import { useStrings } from '@/shell/strings'
import type { Run, Signal } from '@/types'

export function SignalBadges({ signals, limit }: { signals: Signal[]; limit?: number }) {
  const labels = useStrings().signals
  const ordered = SIGNAL_ORDER.filter((signal) => signals.includes(signal))
  const shown = limit === undefined ? ordered : ordered.slice(0, limit)
  const hidden = ordered.slice(shown.length)
  return (
    <>
      {shown.map((signal) => {
        const style = SIGNALS[signal]
        const Icon = style.icon
        return (
          <Badge key={signal} variant={style.variant}>
            <Icon />
            {labels[signal]}
          </Badge>
        )
      })}
      {hidden.length ? (
        <Badge variant="secondary" title={hidden.map((signal) => labels[signal]).join(', ')}>
          +{hidden.length}
        </Badge>
      ) : null}
    </>
  )
}

const STATUS_STYLE: Record<Run['status'], { icon: typeof CircleCheck; text: string; chip: string }> = {
  ok: { icon: CircleCheck, text: 'text-success', chip: 'bg-success/10 text-success' },
  failed: { icon: CircleX, text: 'text-danger', chip: 'bg-danger/10 text-danger' },
  cancelled: { icon: Ban, text: 'text-muted-foreground', chip: 'bg-muted text-muted-foreground' },
  running: { icon: Loader, text: 'text-k-llm animate-spin', chip: 'bg-k-llm/10 text-k-llm [&>svg]:animate-spin' },
}

export function StatusIcon({ status, chip = false }: { status: Run['status']; chip?: boolean }) {
  const labels = useStrings().status
  const style = STATUS_STYLE[status]
  const Icon = style.icon
  if (chip) {
    return (
      <span className={cn('grid size-9 shrink-0 place-items-center rounded-lg', style.chip)}>
        <Icon className="size-5" />
      </span>
    )
  }
  return <Icon className={cn('size-4', style.text)} aria-label={labels[status]} />
}
