// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import ReadOnlyField from './ReadOnlyField.svelte';

describe('copyable read-only information', () => {
  it('copies the full selectable identity and reports authoritative clipboard success', async () => {
    let complete!: () => void;
    const writeText = vi.fn(() => new Promise<void>((resolve) => (complete = resolve)));
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    const target = document.createElement('div');
    const component = mount(ReadOnlyField, {
      target,
      props: {
        label: 'Server identity',
        value: 'FULL-FINGERPRINT',
        copyable: true,
        technical: true,
      },
    });
    const button = target.querySelector('button')!;
    expect(target.querySelector('input')).toBeNull();
    expect(target.querySelector('code')?.textContent).toBe('FULL-FINGERPRINT');
    button.click();
    await tick();
    expect(button.disabled).toBe(true);
    expect(target.querySelector('[role="status"]')).toBeNull();
    expect(writeText).toHaveBeenCalledWith('FULL-FINGERPRINT');
    complete();
    await vi.waitFor(() => {
      expect(target.querySelector('[role="status"]')?.textContent).toContain('copied');
    });
    await unmount(component);
  });

  it('surfaces clipboard rejection without claiming success or hiding the value', async () => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: vi.fn().mockRejectedValue(new Error('Access denied')) },
    });
    const target = document.createElement('div');
    const component = mount(ReadOnlyField, {
      target,
      props: { label: 'Server identity', value: 'FULL-FINGERPRINT', copyable: true },
    });
    target.querySelector('button')!.click();
    await vi.waitFor(() => {
      expect(target.querySelector('[role="alert"]')?.textContent).toContain('copy it manually');
    });
    expect(target.querySelector('[role="status"]')).toBeNull();
    expect(target.textContent).toContain('FULL-FINGERPRINT');
    await unmount(component);
  });
});
