import animate from 'tailwindcss-animate'

const token = (name) => `hsl(var(--${name}) / <alpha-value>)`
const pair = (name, fg) => ({ DEFAULT: token(name), foreground: token(fg ?? `${name}-foreground`) })

export default {
  darkMode: 'media',
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        border: token('border'),
        input: token('input'),
        ring: token('ring'),
        background: token('background'),
        foreground: token('foreground'),
        sidebar: token('sidebar'),
        success: token('success'),
        warning: token('warning'),
        danger: token('danger'),
        primary: pair('primary'),
        secondary: pair('secondary'),
        muted: pair('muted'),
        accent: pair('accent'),
        card: pair('card'),
        popover: pair('popover', 'foreground'),
        k: {
          run: token('k-run'),
          agent: token('k-agent'),
          llm: token('k-llm'),
          tool: token('k-tool'),
          task: token('k-task'),
          eval: token('k-eval'),
          wait: token('k-wait'),
          event: token('k-event'),
        },
      },
      borderRadius: {
        xl: 'calc(var(--radius) + 4px)',
        lg: 'var(--radius)',
        md: 'calc(var(--radius) - 2px)',
        sm: 'calc(var(--radius) - 4px)',
      },
      fontFamily: {
        sans: ['Geist', 'ui-sans-serif', 'system-ui', '-apple-system', '"Segoe UI"', 'sans-serif'],
        mono: ['"Geist Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'monospace'],
      },
    },
  },
  plugins: [animate],
}
