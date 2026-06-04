import { bold, dim, moss } from '../ansi';
import type { Command } from './types';

const listPage = (title: string, ids: string[], how: string): string =>
  [bold(title), ...ids.map((id) => '  ' + moss(id)), '', dim(how)].join('\n');

export const soundCommand: Command = {
  name: 'sound',
  aliases: ['soundscape'],
  summary: 'an ambient soundscape (sound <name> | off)',
  run: async (args, ctx) => {
    const arg = args[0]?.toLowerCase();
    if (!arg) {
      const list = await ctx.audio.listSoundscapes();
      if (list.length === 0) {
        ctx.status('no soundscapes available right now');
        return;
      }
      ctx.page(listPage('soundscapes', list.map((a) => a.id), 'play one with: sound <name>'));
      return;
    }
    if (arg === 'off' || arg === 'stop') {
      ctx.audio.stopSoundscape();
      ctx.setSound(null);
      ctx.status('sound off');
      return;
    }
    ctx.setSound(arg);
    ctx.status(`sound · ${arg}`);
    await ctx.audio.playSoundscape(arg);
  },
};

export const voiceCommand: Command = {
  name: 'voice',
  summary: 'a meditation voice guide (voice <name> | off)',
  run: async (args, ctx) => {
    const arg = args[0]?.toLowerCase();
    if (!arg) {
      const list = await ctx.audio.listVoices();
      if (list.length === 0) {
        ctx.status('no voice guides available right now');
        return;
      }
      ctx.page(listPage('voices', list.map((p) => p.id), 'play one with: voice <name>'));
      return;
    }
    if (arg === 'off' || arg === 'stop') {
      ctx.audio.stopVoice();
      ctx.status('voice off');
      return;
    }
    ctx.status(`voice · ${arg}`);
    await ctx.audio.playVoice(arg);
  },
};

export const bellCommand: Command = {
  name: 'bell',
  summary: 'ring a bell (bell <name> for a downloaded one)',
  run: async (args, ctx) => {
    await ctx.audio.ring(args[0]?.toLowerCase());
    ctx.status('∿ bell');
  },
};
