// Prepare bundled binaries for Tauri resources.
// Copies bootkeeper.exe and bootkeeper-helper.exe from the workspace
// release target into src-tauri/resources so the bundle includes them.
// Run as part of beforeBuildCommand (after cargo build --release).

import { cpSync, mkdirSync, existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const here = dirname(fileURLToPath(import.meta.url))
const targetDir = join(here, '..', '..', 'target', 'release')
const outDir = join(here, 'resources')

mkdirSync(outDir, { recursive: true })

const bins = ['bootkeeper.exe', 'bootkeeper-helper.exe']
let copied = 0
for (const bin of bins) {
  const src = join(targetDir, bin)
  if (existsSync(src)) {
    cpSync(src, join(outDir, bin))
    copied++
    console.log(`resource: ${bin}`)
  } else {
    console.warn(`missing: ${src}`)
  }
}

if (copied === 0) {
  console.error('no binaries copied — did cargo build --release run first?')
  process.exit(1)
}
