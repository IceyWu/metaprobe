import { execFileSync } from 'node:child_process'

if (process.env.METAPROBE_SKIP_BUILD !== '1') {
  if (process.platform === 'win32') {
    execFileSync(process.env.ComSpec ?? 'cmd.exe', ['/d', '/s', '/c', 'pnpm build'], {
      stdio: 'inherit',
    })
  } else {
    execFileSync('pnpm', ['build'], { stdio: 'inherit' })
  }
}

execFileSync(process.execPath, ['scripts/verify-package.mjs'], { stdio: 'inherit' })
