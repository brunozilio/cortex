import { readFileSync, writeFileSync, statSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const here = path.dirname(fileURLToPath(import.meta.url))
const built = path.resolve(here, '../dist/index.html')
const target = path.resolve(here, '../../../crates/cli/src/tui/features/trace/trace.html')
const html = readFileSync(built, 'utf8')

const count = (needle) => html.split(needle).length - 1
const problems = []
if (count('{{TRACE}}') !== 1) problems.push(`{{TRACE}} appears ${count('{{TRACE}}')} times`)
if (count('{{SESSION}}') !== 1) problems.push(`{{SESSION}} appears ${count('{{SESSION}}')} times`)
for (const banned of ['src="http', 'href="http', '@import']) {
  if (html.includes(banned)) problems.push(`bundle contains ${banned}`)
}
if (problems.length) {
  console.error(problems.join('\n'))
  process.exit(1)
}

writeFileSync(target, html)
console.log(`wrote ${path.relative(process.cwd(), target)} (${(statSync(target).size / 1024).toFixed(1)} KB)`)
