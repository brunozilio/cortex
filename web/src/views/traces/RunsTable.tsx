import { useEffect, useMemo, useState } from 'react'
import { ChevronLeft, ChevronRight, Search, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { SignalBadges, StatusIcon } from '@/components/RunBadges'
import { FacetFilter } from './FacetFilter'
import { applyFilters, EMPTY_FILTERS, facetsFor, type Filters } from '@/lib/filters'
import { ms, percent, shortId, tokens, usd, when } from '@/lib/format'
import { runKey, type RunRef } from '@/lib/report'
import { cn } from '@/lib/utils'
import type { Run } from '@/types'

const PAGE_SIZE = 10
const MAX_SIGNALS = 2

interface RunsTableProps {
  runs: Run[]
  current: string
  nowMs: number
  selected: RunRef | null
  filters: Filters
  onFilters: (filters: Filters) => void
  onSelect: (run: Run) => void
}

export function RunsTable({ runs, current, nowMs, selected, filters, onFilters, onSelect }: RunsTableProps) {
  const facets = useMemo(() => facetsFor(runs, current).filter((facet) => facet.key !== 'session'), [runs, current])
  const visible = useMemo(() => applyFilters(runs, filters, ''), [runs, filters])
  const [page, setPage] = useState(0)
  const pages = Math.max(1, Math.ceil(visible.length / PAGE_SIZE))
  useEffect(() => setPage(0), [filters])
  const shown = visible.slice(page * PAGE_SIZE, page * PAGE_SIZE + PAGE_SIZE)
  const maxCost = Math.max(...runs.map((run) => run.cost_usd), 0.0001)
  const selectedKey = selected ? runKey(selected) : ''
  const reset = () => onFilters({ ...EMPTY_FILTERS, session: filters.session })
  const filtered = Object.entries(filters).some(([key, values]) => key !== 'session' && values.length > 0)
  return (
    <Card>
      <div className="flex flex-wrap items-center gap-2 p-4">
        {facets.map((facet) => (
          <FacetFilter
            key={facet.key}
            facet={facet}
            runs={runs}
            selected={filters[facet.key]}
            onChange={(values) => onFilters({ ...filters, [facet.key]: values })}
          />
        ))}
        {filtered ? (
          <Button variant="ghost" onClick={reset}>
            Reset
            <X />
          </Button>
        ) : null}
      </div>
      {shown.length ? (
        <>
          <Table>
            <TableHeader>
              <TableRow className="border-t">
                <TableHead className="w-8 pl-4 pr-1" />
                <TableHead className="px-3">Prompt</TableHead>
                <TableHead className="px-3">Model</TableHead>
                <TableHead className="px-3 text-right">Duration</TableHead>
                <TableHead className="px-3 text-right">Tokens</TableHead>
                <TableHead className="px-3 text-right">Cost</TableHead>
                <TableHead className="px-3 text-right">Waste</TableHead>
                <TableHead className="px-3 pr-4">Signals</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {shown.map((run) => (
                <TableRow
                  key={runKey(run)}
                  tabIndex={0}
                  data-state={runKey(run) === selectedKey ? 'selected' : undefined}
                  onClick={() => onSelect(run)}
                  onKeyDown={(event) => event.key === 'Enter' && onSelect(run)}
                  className="cursor-pointer"
                >
                  <TableCell className="pl-4 pr-1">
                    <StatusIcon status={run.status} />
                  </TableCell>
                  <TableCell className="min-w-[260px] max-w-[440px] px-3">
                    <div className="truncate font-medium">{run.prompt}</div>
                    <div className="text-xs text-muted-foreground">
                      session <span className="font-mono">{shortId(run.session)}</span>
                      {run.session === current ? ' (current)' : ''} · run {run.index} · {when(run.started_at_ms, nowMs)}
                    </div>
                  </TableCell>
                  <TableCell className="whitespace-nowrap px-3 text-muted-foreground">{run.model}</TableCell>
                  <TableCell className="num whitespace-nowrap px-3 text-right">{ms(run.duration_ms)}</TableCell>
                  <TableCell className="num px-3 text-right">{tokens(run.tokens)}</TableCell>
                  <TableCell className="px-3">
                    <div className="flex items-center justify-end gap-2">
                      <div className="hidden h-1.5 w-12 overflow-hidden rounded-full bg-muted sm:block">
                        <div className="h-full bg-foreground/70" style={{ width: `${(run.cost_usd / maxCost) * 100}%` }} />
                      </div>
                      <span className="num font-medium">{usd(run.cost_usd)}</span>
                    </div>
                  </TableCell>
                  <TableCell className={cn('num px-3 text-right', run.waste > 0.2 ? 'font-medium text-warning' : 'text-muted-foreground')}>
                    {Math.round(run.waste * 100) === 0 ? <span aria-label="none">–</span> : percent(run.waste)}
                  </TableCell>
                  <TableCell className="px-3 pr-4">
                    <div className="flex gap-1 whitespace-nowrap">
                      <SignalBadges signals={run.signals} limit={MAX_SIGNALS} />
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          <div className="flex items-center justify-between border-t px-4 py-3 text-sm text-muted-foreground">
            <span>
              {visible.length} of {runs.length} runs
              {pages > 1 ? ` · page ${page + 1} of ${pages}` : ''}
            </span>
            <div className="flex gap-2">
              <Button size="icon" aria-label="Previous page" disabled={page === 0} onClick={() => setPage(page - 1)}>
                <ChevronLeft />
              </Button>
              <Button size="icon" aria-label="Next page" disabled={page >= pages - 1} onClick={() => setPage(page + 1)}>
                <ChevronRight />
              </Button>
            </div>
          </div>
        </>
      ) : (
        <div className="grid place-items-center gap-2 border-t p-10 text-center text-sm text-muted-foreground">
          <Search className="size-6" />
          No runs match these filters.
          <Button onClick={reset}>Reset filters</Button>
        </div>
      )}
    </Card>
  )
}
