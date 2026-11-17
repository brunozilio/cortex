import { Children, isValidElement, useMemo, type ReactElement, type ReactNode } from 'react'
import ReactMarkdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import { AlertTriangle, FlaskConical, Info, Lightbulb, Link as LinkIcon, OctagonAlert, type LucideIcon } from 'lucide-react'
import { CopyButton } from '@/components/CopyButton'
import { headings } from '@/docs/headings'
import { say } from '@/docs/i18n'
import type { Lang } from '@/docs/pages'
import { internalTarget } from '@/docs/route'
import { cn } from '@/lib/utils'

type Kind = 'NOTE' | 'TIP' | 'IMPORTANT' | 'WARNING' | 'CAUTION' | 'EXPERIMENTAL'

const KINDS: Record<Kind, { icon: LucideIcon; box: string; tone: string }> = {
  NOTE: { icon: Info, box: 'border-l-[hsl(var(--chart-1))] bg-[hsl(var(--chart-1)/0.07)]', tone: 'text-[hsl(var(--chart-1))]' },
  TIP: { icon: Lightbulb, box: 'border-l-success bg-success/[0.07]', tone: 'text-success' },
  IMPORTANT: { icon: Info, box: 'border-l-k-agent bg-k-agent/[0.07]', tone: 'text-k-agent' },
  WARNING: { icon: AlertTriangle, box: 'border-l-warning bg-warning/[0.07]', tone: 'text-warning' },
  CAUTION: { icon: OctagonAlert, box: 'border-l-danger bg-danger/[0.07]', tone: 'text-danger' },
  EXPERIMENTAL: { icon: FlaskConical, box: 'border-l-k-eval bg-k-eval/[0.07]', tone: 'text-k-eval' },
}

function textOf(node: ReactNode): string {
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(textOf).join('')
  if (isValidElement(node)) return textOf((node as ReactElement<{ children?: ReactNode }>).props.children)
  return ''
}

function calloutKind(children: ReactNode): { kind: Kind; rest: ReactNode[] } | null {
  const items = Children.toArray(children).filter((child) => !(typeof child === 'string' && !child.trim()))
  const first = items[0]
  if (!isValidElement(first)) return null
  const inner = Children.toArray((first as ReactElement<{ children?: ReactNode }>).props.children)
  const marker = /^\s*\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION|EXPERIMENTAL)\]\s*/.exec(typeof inner[0] === 'string' ? inner[0] : '')
  if (!marker) return null
  const lead = (inner[0] as string).slice(marker[0].length)
  const firstRest = [lead, ...inner.slice(1)].filter((part) => part !== '')
  const rest = firstRest.length ? [<p key="lead">{firstRest}</p>, ...items.slice(1)] : items.slice(1)
  return { kind: marker[1] as Kind, rest }
}

interface MarkdownProps {
  body: string
  lang: Lang
  onLink: (slug: string, id: string) => void
}

export function Markdown({ body, lang, onLink }: MarkdownProps) {
  const ids = useMemo(() => headings(body), [body])
  const components = useMemo<Components>(() => {
    const byLine = new Map(ids.map((heading) => [heading.line, heading.id]))
    const heading = (Tag: 'h2' | 'h3') =>
      function Heading({ children, node }: { children?: ReactNode; node?: { position?: { start: { line: number } } } }) {
        const id = byLine.get(node?.position?.start.line ?? 0) ?? ''
        return (
          <Tag id={id} className="group scroll-mt-20">
            {children}
            <a href={`#${id}`} className="ml-2 inline-block opacity-0 transition-opacity group-hover:opacity-60" aria-label="#">
              <LinkIcon className="inline size-4" />
            </a>
          </Tag>
        )
      }
    return {
      h2: heading('h2'),
      h3: heading('h3'),
      a({ href = '', children }) {
        const target = internalTarget(href)
        if (!target) return <a href={href} target="_blank" rel="noreferrer">{children}</a>
        return (
          <a
            href={href}
            onClick={(event) => {
              event.preventDefault()
              onLink(target.slug, target.id)
            }}
          >
            {children}
          </a>
        )
      },
      blockquote({ children }) {
        const callout = calloutKind(children)
        if (!callout) return <blockquote>{children}</blockquote>
        const { icon: Icon, box, tone } = KINDS[callout.kind]
        return (
          <div className={cn('not-prose my-5 rounded-r-lg border-l-4 px-4 py-3 text-sm', box)}>
            <div className={cn('mb-1 flex items-center gap-2 text-xs font-semibold uppercase tracking-wider', tone)}>
              <Icon className="size-4" />
              {say(lang, callout.kind)}
            </div>
            <div className="callout-body">{callout.rest}</div>
          </div>
        )
      },
      pre({ children }) {
        const code = textOf(children).replace(/\n$/, '')
        const language = /language-(\w+)/.exec(
          isValidElement(children) ? ((children as ReactElement<{ className?: string }>).props.className ?? '') : '',
        )?.[1]
        return (
          <div className="group relative my-5">
            {language ? <span className="absolute left-3 top-2 text-[10px] uppercase tracking-wider text-muted-foreground">{language}</span> : null}
            <CopyButton
              text={code}
              label={say(lang, 'copy')}
              iconOnly
              variant="ghost"
              size="icon"
              className="absolute right-2 top-2 size-7 opacity-0 transition-opacity group-hover:opacity-100"
            />
            <pre className={cn(language && 'pt-7')}>{children}</pre>
          </div>
        )
      },
      table({ children }) {
        return (
          <div className="my-5 overflow-x-auto rounded-lg border">
            <table>{children}</table>
          </div>
        )
      },
    }
  }, [ids, lang, onLink])

  return (
    <div className="docs-prose">
      <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[[rehypeHighlight, { detect: false, ignoreMissing: true }]]} components={components}>
        {body}
      </ReactMarkdown>
    </div>
  )
}
