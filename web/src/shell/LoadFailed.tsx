import { Unplug } from 'lucide-react'
import { Card } from '@/components/ui/card'

export function LoadFailed({ title, message }: { title: string; message: string }) {
  return (
    <main className="grid min-h-screen place-items-center px-4">
      <Card role="alert" className="grid max-w-lg justify-items-center gap-3 p-10 text-center">
        <Unplug className="h-6 w-6 text-muted-foreground" aria-hidden />
        <h1 className="text-base font-semibold">{title}</h1>
        <p className="text-sm text-muted-foreground">{message}</p>
      </Card>
    </main>
  )
}
