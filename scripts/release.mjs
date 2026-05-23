#!/usr/bin/env node
import { createInterface } from 'readline'
import { readFileSync, writeFileSync } from 'fs'
import { execSync } from 'child_process'
import { fileURLToPath } from 'url'
import { dirname, resolve } from 'path'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

// Cargo.toml is the version source of truth (tauri.conf.json inherits it automatically)
const cargoPath = resolve(root, 'src-tauri/Cargo.toml')
const cargo = readFileSync(cargoPath, 'utf8')
const current = cargo.match(/\[package\][^[]*?version\s*=\s*"([^"]*)"/s)?.[1]
if (!current) {
  console.error('Could not read version from Cargo.toml. Aborting.')
  process.exit(1)
}

const [major, minor, patch] = current.split('.').map(Number)

const candidates = {
  patch: `${major}.${minor}.${patch + 1}`,
  minor: `${major}.${minor + 1}.0`,
  major: `${major + 1}.0.0`,
}

console.log(`\nCurrent version: ${current}\n`)
console.log(`  1) patch  →  ${candidates.patch}`)
console.log(`  2) minor  →  ${candidates.minor}`)
console.log(`  3) major  →  ${candidates.major}\n`)

const rl = createInterface({ input: process.stdin, output: process.stdout })

rl.question('Bump type (1/2/3 or patch/minor/major): ', (answer) => {
  rl.close()

  const lookup = {
    1: 'patch',
    patch: 'patch',
    2: 'minor',
    minor: 'minor',
    3: 'major',
    major: 'major',
  }
  const type = lookup[answer.trim().toLowerCase()]

  if (!type) {
    console.error('Invalid choice. Aborting.')
    process.exit(1)
  }

  const next = candidates[type]
  const tag = `v${next}`
  console.log(`\nBumping to ${next}...\n`)

  // ── package.json ──────────────────────────────────────────────────────────
  const pkgPath = resolve(root, 'package.json')
  const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'))
  pkg.version = next
  writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n')

  // ── src-tauri/Cargo.toml ──────────────────────────────────────────────────
  const updatedCargo = cargo.replace(/(\[package\][^[]*?version\s*=\s*)"[^"]*"/s, `$1"${next}"`)

  if (updatedCargo === cargo) {
    console.error('Could not locate version field in Cargo.toml. Aborting.')
    process.exit(1)
  }

  writeFileSync(cargoPath, updatedCargo)

  // ── git: commit → tag → push ──────────────────────────────────────────────
  try {
    execSync('git add package.json src-tauri/Cargo.toml', {
      cwd: root,
      stdio: 'inherit',
    })
    execSync(`git commit -m "chore: release ${tag}"`, { cwd: root, stdio: 'inherit' })
    execSync(`git tag ${tag}`, { cwd: root, stdio: 'inherit' })
    execSync('git push', { cwd: root, stdio: 'inherit' })
    execSync(`git push origin ${tag}`, { cwd: root, stdio: 'inherit' })
    console.log(`\nDone — ${tag} pushed. GitHub Actions will build the release.`)
  } catch {
    console.error('\nGit step failed. Version files have been updated locally.')
    process.exit(1)
  }
})
