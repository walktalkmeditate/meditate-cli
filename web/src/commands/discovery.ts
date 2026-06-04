import { bold, dim, moss } from '../ansi';
import type { Command } from './types';

// The canonical install commands — kept VERBATIM and identical to the README so
// a copied line is trustworthy (anti-malware: what you see is what you run).
const BREW = 'brew install walktalkmeditate/tap/meditate';
const CURL =
  'curl -fsSL https://raw.githubusercontent.com/walktalkmeditate/meditate-cli/main/install.sh | sh';
const PWSH =
  'irm https://raw.githubusercontent.com/walktalkmeditate/meditate-cli/main/install.ps1 | iex';

function installPage(copied: boolean): string {
  return [
    bold('run meditate in your real terminal'),
    '',
    dim('  macOS · Homebrew'),
    `    ${moss(BREW)}`,
    '',
    dim('  macOS / Linux'),
    `    ${moss(CURL)}`,
    '',
    dim('  Windows · PowerShell'),
    `    ${moss(PWSH)}`,
    '',
    copied
      ? dim('  (copied the Homebrew line to your clipboard — exactly what you see)')
      : dim('  select a line to copy it — exactly what you see, nothing hidden'),
  ].join('\n');
}

async function copyBrew(): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(BREW);
    return true;
  } catch {
    return false;
  }
}

export const installCommand: Command = {
  name: 'install',
  aliases: ['brew'],
  summary: 'run it in your real terminal',
  run: async (_args, ctx) => {
    // The command was typed — a user gesture — so the clipboard write is allowed.
    const copied = await copyBrew();
    ctx.page(installPage(copied));
  },
};

export const whichCommand: Command = {
  name: 'which',
  summary: 'is meditate installed here?',
  hidden: true,
  run: (_args, ctx) => {
    ctx.status("meditate: not installed in this tab — type 'install' to get the real thing");
  },
};

export const whoamiCommand: Command = {
  name: 'whoami',
  aliases: ['credits'],
  summary: 'who is behind this',
  hidden: true,
  run: (_args, ctx) => {
    ctx.page(
      [
        bold('you') + dim(' · a quiet breather in a browser tab'),
        '',
        '  no account, no tracking — your streak lives only on this device.',
        '',
        dim('  made by the folks behind ') + moss('Pilgrim') + dim(', a walking-meditation app.'),
        dim('  pilgrimapp.org'),
      ].join('\n'),
    );
  },
};
