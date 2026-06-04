// The touch chip row: every essential action reachable without a keyboard.
// Tapping a chip runs the equivalent command (so the user sees what it maps to)
// and counts as the interaction gesture that unlocks audio.

export interface Chip {
  label: string;
  command: string;
}

/** A coarse pointer (finger) — show the chip row. */
export function isTouch(): boolean {
  return window.matchMedia('(pointer: coarse)').matches;
}

export function createChipBar(
  chips: Chip[],
  onCommand: (command: string) => void,
): HTMLElement {
  const bar = document.createElement('div');
  bar.id = 'chips';
  bar.setAttribute('role', 'toolbar');
  bar.setAttribute('aria-label', 'meditate controls');

  for (const chip of chips) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'chip';
    button.textContent = chip.label;
    button.addEventListener('click', () => onCommand(chip.command));
    bar.appendChild(button);
  }
  return bar;
}
