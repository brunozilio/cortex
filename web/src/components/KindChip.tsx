import { KINDS } from '@/lib/kinds'
import { cn } from '@/lib/utils'
import type { SpanKind } from '@/types'

export function KindChip({ kind, size = 'sm' }: { kind: SpanKind; size?: 'sm' | 'md' }) {
  const style = KINDS[kind]
  const Icon = style.icon
  return (
    <span className={cn('grid shrink-0 place-items-center rounded-md', style.chip, size === 'md' ? 'size-9 rounded-lg' : 'size-6')}>
      <Icon className={size === 'md' ? 'size-4' : 'size-3.5'} />
    </span>
  )
}

export function KindSwatch({ kind, className }: { kind: SpanKind; className?: string }) {
  return <span className={cn('size-2.5 shrink-0 rounded-[3px]', KINDS[kind].swatch, className)} />
}
