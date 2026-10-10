// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import type { Picture } from '../model';
import ImageEditor from './ImageEditor.svelte';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('interactive URL image editor', () => {
  it('offers URL-only input and directly manipulates the crop', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const value: Picture = {
      url: 'https://images.example/avatar.webp',
      data: 'data:image/png;base64,aGVsbG8=',
      shape: 'circle',
      x: 50,
      y: 50,
      zoom: 1,
    };
    const component = mount(ImageEditor, { target, props: { value, title: 'Profile picture' } });
    const crop = target.querySelector<HTMLButtonElement>('.crop-viewport')!;
    expect(target.querySelector('input[type="file"]')).toBeNull();
    expect(target.querySelector<HTMLInputElement>('input[type="url"]')?.value).toBe(value.url);
    const image = crop.querySelector<HTMLImageElement>('img')!;
    crop.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    await tick();
    expect(image.getAttribute('style')).toContain('51% 50%');

    await unmount(component);
    target.remove();
  });
});
