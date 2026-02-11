#!/usr/bin/env node

import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const outputDir = path.join(repoRoot, 'example', 'bin');

const backend = process.env.LLAMA_BACKEND;
if (!backend) {
  console.error('[llama-prep] LLAMA_BACKEND is required (e.g. win-cpu-x64, macos-arm64).');
  process.exit(1);
}

const dryRun = process.argv.includes('--dry-run');
const repo = process.env.LLAMA_CPP_REPO || 'ggml-org/llama.cpp';
const tag = process.env.LLAMA_CPP_TAG?.trim() || null;

const apiBase = `https://api.github.com/repos/${repo}/releases`;
const releaseUrl = tag ? `${apiBase}/tags/${encodeURIComponent(tag)}` : `${apiBase}/latest`;

function getAssetPatterns(target) {
  switch (target) {
    case 'win-cpu-x64':
      return [/^llama-.*-bin-win-cpu-x64\.zip$/i, /^llama-.*-bin-win-common_cpus-x64\.zip$/i];
    case 'win-arm64':
      return [/^llama-.*-bin-win-cpu-arm64\.zip$/i, /^llama-.*-bin-win-arm64\.zip$/i];
    case 'ubuntu-x64':
      return [
        /^llama-.*-bin-ubuntu-x64\.tar\.gz$/i,
        /^llama-.*-bin-linux-x64\.tar\.gz$/i,
        /^llama-.*-bin-linux-common_cpus-x64\.tar\.gz$/i,
      ];
    case 'macos-arm64':
      return [/^llama-.*-bin-macos-arm64\.tar\.gz$/i, /^llama-.*-bin-darwin-arm64\.tar\.gz$/i];
    case 'macos-x64':
      return [/^llama-.*-bin-macos-x64\.tar\.gz$/i, /^llama-.*-bin-darwin-x64\.tar\.gz$/i];
    default:
      throw new Error(`Unsupported LLAMA_BACKEND "${target}"`);
  }
}

function runCommand(command, args, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, stdio: 'inherit' });
    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${command} exited with code ${code}`));
      }
    });
  });
}

function runCommandCapture(command, args, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });
    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) {
        resolve({ stdout, stderr });
      } else {
        reject(new Error(`${command} exited with code ${code}: ${stderr}`));
      }
    });
  });
}

async function fetchRelease() {
  const args = [
    '-fsSL',
    '-A',
    'oxide-lab-ci-llama-prep',
    '-H',
    'Accept: application/vnd.github+json',
  ];
  if (process.env.GITHUB_TOKEN) {
    args.push('-H', `Authorization: Bearer ${process.env.GITHUB_TOKEN}`);
  }
  args.push(releaseUrl);
  const { stdout } = await runCommandCapture('curl', args, repoRoot);
  return JSON.parse(stdout);
}

async function downloadToFile(url, destination) {
  await fs.mkdir(path.dirname(destination), { recursive: true });
  const args = [
    '-fL',
    '--retry',
    '3',
    '--retry-delay',
    '2',
    '--retry-connrefused',
    '-A',
    'oxide-lab-ci-llama-prep',
    '-H',
    'Accept: application/octet-stream',
  ];
  if (process.env.GITHUB_TOKEN) {
    args.push('-H', `Authorization: Bearer ${process.env.GITHUB_TOKEN}`);
  }
  args.push('-o', destination, url);
  await runCommand('curl', args, repoRoot);
}

async function fileExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function findLlamaServer(rootDir) {
  const queue = [rootDir];
  while (queue.length > 0) {
    const current = queue.pop();
    const entries = await fs.readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      const fullPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        queue.push(fullPath);
        continue;
      }
      const lower = entry.name.toLowerCase();
      if (lower === 'llama-server' || lower === 'llama-server.exe') {
        return fullPath;
      }
    }
  }
  return null;
}

async function main() {
  console.log(`[llama-prep] backend=${backend}`);
  console.log(`[llama-prep] release=${tag ?? 'latest'}`);

  const release = await fetchRelease();
  const patterns = getAssetPatterns(backend);

  const assets = Array.isArray(release.assets) ? release.assets : [];
  const selected = patterns
    .map((pattern) => assets.find((asset) => pattern.test(asset.name)))
    .find(Boolean);

  if (!selected) {
    const available = assets.map((a) => a.name).join('\n');
    throw new Error(
      `[llama-prep] No matching asset for backend "${backend}" in release "${release.tag_name}".\nAvailable assets:\n${available}`,
    );
  }

  console.log(`[llama-prep] release_tag=${release.tag_name}`);
  console.log(`[llama-prep] asset=${selected.name}`);

  if (dryRun) {
    return;
  }

  await fs.mkdir(outputDir, { recursive: true });
  const archivePath = path.join(outputDir, selected.name);

  if (!(await fileExists(archivePath))) {
    console.log(`[llama-prep] downloading ${selected.browser_download_url}`);
    await downloadToFile(selected.browser_download_url, archivePath);
  } else {
    console.log(`[llama-prep] using cached archive ${archivePath}`);
  }

  console.log(`[llama-prep] extracting ${selected.name}`);
  await runCommand('tar', ['-xf', archivePath, '-C', outputDir], repoRoot);

  const serverPath = await findLlamaServer(outputDir);
  if (!serverPath) {
    throw new Error('[llama-prep] Extraction finished but llama-server binary was not found');
  }
  console.log(`[llama-prep] ready: ${serverPath}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
