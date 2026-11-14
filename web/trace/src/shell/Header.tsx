import { forwardRef } from 'react'
import { Calendar, Search } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { shortId } from '@/lib/format'
import type { LiveStatus } from '@/lib/live'
import { NAV, type View } from './Sidebar'

interface HeaderProps {
  project: string
  current: string
  live: LiveStatus
  view: View
  query: string
  onView: (view: View) => void
  onQuery: (query: string) => void
}

function LiveBadge({ status, current }: { status: LiveStatus; current: string }) {
  if (status === 'reconnecting') {
    return (
      <Badge variant="warning" className="hidden sm:inline-flex">
        <span className="size-1.5 animate-pulse rounded-full bg-warning" />
        Reconnecting…
      </Badge>
    )
  }
  if (status === 'live') {
    return (
      <Badge variant="success" className="hidden sm:inline-flex">
        <span className="pulse-dot size-1.5 rounded-full bg-success" />
        Live
      </Badge>
    )
  }
  if (status === 'connecting') {
    return (
      <Badge variant="secondary" className="hidden sm:inline-flex">
        <span className="size-1.5 rounded-full bg-muted-foreground" />
        Connecting…
      </Badge>
    )
  }
  return (
    <Badge variant="success" className="hidden sm:inline-flex">
      <span className="pulse-dot size-1.5 rounded-full bg-success" />
      current {shortId(current)}
    </Badge>
  )
}

export const Header = forwardRef<HTMLInputElement, HeaderProps>(({ project, current, live, view, query, onView, onQuery }, searchRef) => (
  <header className="sticky top-0 z-20 border-b bg-background/80 backdrop-blur">
    <div className="flex h-14 items-center gap-3 px-4 md:px-6">
      <nav className="flex min-w-0 items-center gap-1.5 text-sm text-muted-foreground">
        <span className="hidden sm:inline">{project}</span>
        <span className="hidden sm:inline">/</span>
        <span className="truncate font-medium text-foreground">{NAV.find((entry) => entry.key === view)?.label}</span>
      </nav>
      <div className="ml-auto flex items-center gap-2">
        <label className="relative hidden lg:block">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input ref={searchRef} value={query} onChange={(event) => onQuery(event.target.value)} placeholder="Search runs…" className="w-56 pl-8 pr-10" />
          <kbd className="absolute right-2 top-1/2 -translate-y-1/2 rounded border bg-muted px-1.5 font-mono text-[10px] text-muted-foreground">/</kbd>
        </label>
        <Badge className="h-8 gap-2 px-3 text-sm font-medium shadow-sm">
          <Calendar className="!size-3.5" />
          <span className="hidden sm:inline">Last 30 days</span>
        </Badge>
        <LiveBadge status={live} current={current} />
      </div>
    </div>
    <div className="px-4 pb-2 md:hidden">
      <Tabs value={view} onValueChange={(value) => onView(value as View)}>
        <TabsList className="w-full">
          {NAV.map(({ key, label }) => (
            <TabsTrigger key={key} value={key} className="flex-1">
              {label}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>
    </div>
  </header>
))
Header.displayName = 'Header'
