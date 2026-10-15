// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import SideSurface from './SideSurface.svelte';

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});
describe('responsive side surface', () => {
  it.each([false, true])(
    'uses native modal semantics only for narrow=%s and restores focus',
    async (narrow) => {
      let resize!: () => void;
      const media = {
        matches: narrow,
        addEventListener: vi.fn((_event, callback) => {
          resize = callback;
        }),
        removeEventListener: vi.fn(),
      };
      vi.stubGlobal(
        'matchMedia',
        vi.fn(() => media),
      );
      const show = vi.fn(function (this: HTMLDialogElement) {
        this.setAttribute('open', '');
      });
      const showModal = vi.fn(function (this: HTMLDialogElement) {
        this.setAttribute('open', '');
      });
      Object.assign(HTMLDialogElement.prototype, {
        show,
        showModal,
        close(this: HTMLDialogElement) {
          this.removeAttribute('open');
        },
      });
      const opener = document.createElement('button');
      const target = document.createElement('div');
      document.body.append(opener, target);
      opener.focus();
      const close = vi.fn();
      const component = mount(SideSurface, {
        target,
        props: { title: 'Conversation information', close },
      });
      await tick();
      expect(narrow ? showModal : show).toHaveBeenCalledOnce();
      expect(narrow ? show : showModal).not.toHaveBeenCalled();
      const dialog = target.querySelector('dialog')!;
      if (narrow) {
        const first = document.createElement('button');
        const last = document.createElement('button');
        for (const button of [first, last])
          Object.defineProperty(button, 'getClientRects', { value: () => [{}] });
        dialog.append(first, last);
        first.focus();
        first.dispatchEvent(
          new KeyboardEvent('keydown', {
            key: 'Tab',
            shiftKey: true,
            bubbles: true,
            cancelable: true,
          }),
        );
        expect(document.activeElement).toBe(last);
        last.dispatchEvent(
          new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }),
        );
        expect(document.activeElement).toBe(first);
      }
      dialog.dispatchEvent(new Event('cancel', { cancelable: true }));
      expect(close).toHaveBeenCalledOnce();
      media.matches = !narrow;
      resize();
      expect(narrow ? show : showModal).toHaveBeenCalledOnce();
      await unmount(component);
      expect(media.removeEventListener).toHaveBeenCalledWith('change', resize);
      expect(document.activeElement).toBe(opener);
    },
  );
});
