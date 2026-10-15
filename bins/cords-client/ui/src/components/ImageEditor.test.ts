// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import type { Picture } from '../model';
import ImageEditor from './ImageEditor.svelte';
import { importImage } from '../images';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../images', () => ({ importImage: vi.fn() }));

describe('local profile image editor', () => {
  it('offers local input without URL controls and directly manipulates the crop', async () => {
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
    expect(target.querySelector('input[type="file"]')).not.toBeNull();
    expect(target.querySelector('input[type="url"]')).toBeNull();
    expect(target.textContent).not.toContain('Load URL');
    const image = crop.querySelector<HTMLImageElement>('img')!;
    crop.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    await tick();
    expect(image.getAttribute('style')).toContain('51% 50%');

    await unmount(component);
    target.remove();
  });
  it('loads a selected local file into the crop preview without a remote request', async () => {
    vi.mocked(importImage).mockResolvedValue('data:image/png;base64,bG9jYWw=');
    const target = document.createElement('div');
    const value: Picture = { url: '', data: '', shape: 'circle', x: 50, y: 50, zoom: 1 };
    const component = mount(ImageEditor, { target, props: { value, title: 'Profile picture' } });
    const input = target.querySelector<HTMLInputElement>('input[type="file"]')!;
    const file = new File(['pixels'], 'local.png', { type: 'image/png' });
    const bytes = new Uint8Array([137, 80, 78, 71]);
    Object.defineProperty(file, 'arrayBuffer', { value: async () => bytes.buffer });
    Object.defineProperty(input, 'files', { value: [file] });
    input.dispatchEvent(new Event('change', { bubbles: true }));
    for (let i = 0; i < 10; i++) await Promise.resolve();
    await tick();
    expect(importImage).toHaveBeenCalledWith(bytes);
    expect(target.querySelector('img')?.getAttribute('src')).toBe('data:image/png;base64,bG9jYWw=');
    expect(input.value).toBe('');
    await unmount(component);
  });
  it('preserves the existing picture when animation processing is unavailable', async () => {
    vi.mocked(importImage).mockRejectedValue(
      new Error('Animated processing is not available; image was not saved'),
    );
    const target = document.createElement('div');
    const value: Picture = {
      url: '',
      data: 'data:image/png;base64,b2xk',
      shape: 'square',
      x: 25,
      y: 75,
      zoom: 2,
    };
    const component = mount(ImageEditor, { target, props: { value, title: 'Profile picture' } });
    const input = target.querySelector<HTMLInputElement>('input[type="file"]')!;
    const file = new File(['GIF89a'], 'animation.gif');
    Object.defineProperty(file, 'arrayBuffer', {
      value: async () => new Uint8Array([71, 73, 70]).buffer,
    });
    Object.defineProperty(input, 'files', { value: [file] });
    input.dispatchEvent(new Event('change', { bubbles: true }));
    for (let i = 0; i < 10; i++) await Promise.resolve();
    await tick();
    expect(target.querySelector('[role="alert"]')?.textContent).toContain('not saved');
    expect(target.querySelector('img')?.getAttribute('src')).toBe(value.data);
    expect(target.querySelector('img')?.getAttribute('style')).toContain('25% 75%');
    await unmount(component);
  });
});
