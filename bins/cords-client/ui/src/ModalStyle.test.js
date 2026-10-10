import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('modal surface styling', () => {
  it('gives dialogs an opaque app-shell background with explicit fallbacks', () => {
    expect(styles).toMatch(
      /dialog\s*{[^}]*background:\s*var\(--bg-panel,\s*var\(--surface,\s*#34363d\)\);/s,
    );
  });
  it('keeps settings save actions in document flow instead of covering controls', () => {
    const actions = styles.match(/\.settings-actions\s*{([^}]*)}/s)?.[1];
    expect(actions).toBeDefined();
    expect(actions).not.toMatch(/position:\s*sticky|bottom:\s*0/);
  });
});
