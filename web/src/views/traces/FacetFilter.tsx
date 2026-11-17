import { PlusCircle } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Separator } from '@/components/ui/separator'
import { matchesFacet, type Facet } from '@/lib/filters'
import { useStrings } from '@/shell/strings'
import type { Run } from '@/types'

interface FacetFilterProps {
  facet: Facet
  runs: Run[]
  selected: string[]
  onChange: (values: string[]) => void
}

export function FacetFilter({ facet, runs, selected, onChange }: FacetFilterProps) {
  const text = useStrings().traces
  const chosen = facet.options.filter((option) => selected.includes(option.value))
  const toggle = (value: string) => onChange(selected.includes(value) ? selected.filter((entry) => entry !== value) : [...selected, value])
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline" className="border-dashed">
          <PlusCircle />
          {facet.title}
          {chosen.length ? (
            <>
              <Separator orientation="vertical" className="mx-0.5 h-4" />
              {chosen.length > 2 ? (
                <Badge variant="secondary">{text.selected(chosen.length)}</Badge>
              ) : (
                chosen.map((option) => (
                  <Badge key={option.value} variant="secondary">
                    {option.label}
                  </Badge>
                ))
              )}
            </>
          ) : null}
        </Button>
      </PopoverTrigger>
      <PopoverContent>
        <div className="px-2 py-1.5 text-xs font-medium text-muted-foreground">{facet.title}</div>
        {facet.options.map((option) => {
          const id = `facet-${facet.key}-${option.value}`
          const count = runs.filter((run) => matchesFacet(run, facet.key, option.value)).length
          return (
            <label key={option.value} htmlFor={id} className="flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-accent">
              <Checkbox id={id} checked={selected.includes(option.value)} onCheckedChange={() => toggle(option.value)} />
              <span className="truncate">{option.label}</span>
              <span className="ml-auto font-mono text-xs text-muted-foreground">{count}</span>
            </label>
          )
        })}
        {selected.length ? (
          <>
            <Separator className="my-1" />
            <Button variant="ghost" className="w-full" onClick={() => onChange([])}>
              {text.clear}
            </Button>
          </>
        ) : null}
      </PopoverContent>
    </Popover>
  )
}
