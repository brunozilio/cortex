import { useState, type MouseEvent } from 'react'
import { Check, Copy } from 'lucide-react'
import { Button, type ButtonProps } from '@/components/ui/button'
import { useStrings } from '@/shell/strings'

interface CopyButtonProps extends Omit<ButtonProps, 'onClick'> {
  text: string
  label: string
  iconOnly?: boolean
}

async function writeClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    const area = document.createElement('textarea')
    area.value = text
    document.body.appendChild(area)
    area.select()
    const copied = document.execCommand('copy')
    area.remove()
    return copied
  }
}

export function CopyButton({ text, label, iconOnly = false, ...props }: CopyButtonProps) {
  const strings = useStrings()
  const [copied, setCopied] = useState(false)
  const copy = async (event: MouseEvent) => {
    event.stopPropagation()
    if (await writeClipboard(text)) {
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1400)
    }
  }
  return (
    <Button {...props} onClick={copy} aria-label={iconOnly ? label : undefined} title={iconOnly ? label : undefined}>
      {copied ? <Check /> : <Copy />}
      {iconOnly ? null : copied ? strings.inspector.copied : label}
    </Button>
  )
}
