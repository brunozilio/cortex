import { Card } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { KindChip } from '@/components/KindChip'
import { ms, usd } from '@/lib/format'
import { eventTitle } from '@/lib/events'
import { KINDS } from '@/lib/kinds'
import { IoTab } from './IoTab'
import { MetricsTab } from './MetricsTab'
import { MetadataTab } from './MetadataTab'
import { EvalsTab } from './EvalsTab'
import { MessagesTab, SystemTab, ToolsTab } from './MessagesTab'
import type { Finding, Run, Span } from '@/types'

export type InspectorTab = 'io' | 'messages' | 'system' | 'tools' | 'metrics' | 'meta' | 'evals'

const LLM_TABS: InspectorTab[] = ['messages', 'system', 'tools']

const TABS: { key: InspectorTab; label: string }[] = [
  { key: 'io', label: 'Input / Output' },
  { key: 'messages', label: 'Messages' },
  { key: 'system', label: 'System' },
  { key: 'tools', label: 'Tools' },
  { key: 'metrics', label: 'Metrics' },
  { key: 'meta', label: 'Metadata' },
  { key: 'evals', label: 'Evaluations' },
]

interface InspectorProps {
  run: Run
  span: Span
  tab: InspectorTab
  findings: Finding[]
  onTab: (tab: InspectorTab) => void
}

export function Inspector({ run, span, tab, findings, onTab }: InspectorProps) {
  const llm = span.kind === 'llm'
  const tabs = TABS.filter(({ key }) => llm || !LLM_TABS.includes(key))
  const shown = tabs.some(({ key }) => key === tab) ? tab : 'io'
  return (
    <Card>
      <div className="flex items-center gap-3 p-5 pb-4">
        <KindChip kind={span.kind} size="md" />
        <div className="grid min-w-0">
          <span className="text-xs text-muted-foreground">{KINDS[span.kind].label}</span>
          <h3 className="truncate font-semibold">{eventTitle(span)}</h3>
        </div>
        <span className="ml-auto text-right font-mono text-xs text-muted-foreground">
          {span.kind === 'event' ? `at ${ms(span.start_ms)}` : ms(span.end_ms - span.start_ms)}
          {span.usage ? (
            <>
              <br />
              {usd(span.usage.cost_usd)}
            </>
          ) : null}
        </span>
      </div>
      <Tabs value={shown} onValueChange={(value) => onTab(value as InspectorTab)}>
        <div className="px-5">
          <TabsList className="w-full">
            {tabs.map(({ key, label }) => (
              <TabsTrigger key={key} value={key} className="flex-1 text-xs sm:text-sm">
                {label}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>
        <TabsContent value="io" className="grid min-w-0 gap-4 p-5">
          <IoTab span={span} />
        </TabsContent>
        {llm ? (
          <>
            <TabsContent value="messages" className="grid min-w-0 gap-4 p-5">
              <MessagesTab run={run} span={span} />
            </TabsContent>
            <TabsContent value="system" className="grid min-w-0 gap-4 p-5">
              <SystemTab run={run} span={span} />
            </TabsContent>
            <TabsContent value="tools" className="grid min-w-0 gap-4 p-5">
              <ToolsTab run={run} span={span} />
            </TabsContent>
          </>
        ) : null}
        <TabsContent value="metrics" className="grid min-w-0 gap-4 p-5">
          <MetricsTab run={run} span={span} />
        </TabsContent>
        <TabsContent value="meta" className="grid min-w-0 gap-4 p-5">
          <MetadataTab run={run} span={span} />
        </TabsContent>
        <TabsContent value="evals" className="grid min-w-0 gap-4 p-5">
          <EvalsTab run={run} span={span} findings={findings} />
        </TabsContent>
      </Tabs>
    </Card>
  )
}
