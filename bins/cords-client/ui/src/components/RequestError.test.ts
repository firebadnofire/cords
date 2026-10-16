// @vitest-environment jsdom
import { mount, unmount } from 'svelte';
import { describe, expect, it } from 'vitest';
import RequestError from './RequestError.svelte';

describe('request failure presentation', () => {
  it.each([
    ['CORDS_PERMISSION_DENIED: HTTP 403 Forbidden', 'Access restricted'],
    ['connection closed', 'Could not load roles'],
  ])(
    'keeps actionable context and preserves collapsed diagnostics for %s',
    async (error, title) => {
      const target = document.createElement('div');
      const app = mount(RequestError, { target, props: { error, task: 'load roles' } });
      expect(target.querySelector('[role="alert"] strong')?.textContent).toBe(title);
      expect(target.querySelector('details')?.open).toBe(false);
      expect(target.querySelector('code')?.textContent).toBe(error);
      await unmount(app);
    },
  );
});
