import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'
import { useStrings } from '@/shell/strings'

function lineClass(line: string): string {
  if (line.startsWith('+')) return 'text-success'
  if (line.startsWith('-')) return 'text-danger'
  return ''
}

export function CodeBlock({ label, text, action, tall }: { label: string; text: string; action?: ReactNode; tall?: boolean }) {
  const strings = useStrings()
  return (
    <div className="grid min-w-0 gap-1.5">
      <div className="flex min-h-7 items-center justify-between text-xs text-muted-foreground">
        <span className="font-medium uppercase tracking-wider">{label}</span>
        {action}
      </div>
      <pre className={cn('thin-scroll overflow-auto', tall ? 'max-h-[70vh]' : 'max-h-64', ' whitespace-pre-wrap break-words rounded-lg border bg-muted/40 p-3 font-mono text-xs leading-relaxed')}>
        {text
          ? text.split('\n').map((line, i) => (
              <span key={i} className={cn('block', lineClass(line))}>
                {line || ' '}
              </span>
            ))
          : <span className="text-muted-foreground">{strings.inspector.empty}</span>}
      </pre>
    </div>
  )
}
