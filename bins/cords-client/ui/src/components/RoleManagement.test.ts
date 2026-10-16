// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import RoleManagement from './RoleManagement.svelte';
import type { RoleCatalog, RoleRequest } from '../rolePolicy';

const catalog = (): RoleCatalog => ({
  version: 1,
  server_id: 'server',
  revision: 4,
  active: false,
  roles: [
    {
      id: '01a13309-2e49-7981-b6b6-2ad613f221dd',
      name: 'Developers',
      color: '#4488aa',
      description: '',
      priority: 10,
      permissions: [],
      mentionable: false,
    },
  ],
});
async function settle() {
  for (let i = 0; i < 30; i++) await Promise.resolve();
  await tick();
}
function button(target: HTMLElement, text: string) {
  const button = [...target.querySelectorAll('button')].find(
    (item) => item.textContent?.trim() === text,
  );
  if (!button) throw new Error(`Missing button: ${text}`);
  return button;
}
function rename(target: HTMLElement, name: string) {
  const input = target.querySelector<HTMLInputElement>('input[maxlength="64"]');
  if (!input) throw new Error('Role name field missing');
  input.value = name;
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

describe('server-backed role editor', () => {
  it('opens preinitialized roles without submitting a setup request', async () => {
    const state = catalog();
    state.revision = 0;
    state.roles = ['default', 'moderator', 'admin'].map((name, index) => ({
      ...state.roles[0],
      id: `builtin:${name}`,
      name,
      priority: index * 100,
    }));
    const change = vi.fn();
    const target = document.createElement('div');
    const component = mount(RoleManagement, { target, props: { load: async () => state, change } });
    await settle();
    for (const name of ['default', 'moderator', 'admin'])
      expect(target.textContent).toContain(`$${name}`);
    expect(button(target, 'Create role').disabled).toBe(false);
    expect(target.textContent).not.toContain('Initialize role catalog');
    expect(change).not.toHaveBeenCalled();
    await unmount(component);
  });

  it('requests an operator upgrade for an empty legacy catalog instead of client initialization', async () => {
    const change = vi.fn();
    const target = document.createElement('div');
    const component = mount(RoleManagement, {
      target,
      props: { load: async () => ({ ...catalog(), roles: [] }), change },
    });
    await settle();
    expect(target.textContent).toContain('upgrade the server');
    expect(button(target, 'Create role').disabled).toBe(true);
    expect(target.textContent).not.toContain('Initialize role catalog');
    expect(change).not.toHaveBeenCalled();
    await unmount(component);
  });

  it('saves stable identity with the observed revision and reloads authoritative state', async () => {
    let state = catalog();
    const load = vi.fn(async () => state);
    const change = vi.fn(async (request: RoleRequest) => {
      if (request.change.operation !== 'put_role') throw new Error('Unexpected operation');
      state = { ...state, revision: 5, roles: [request.change.role] };
      return { revision: 5, request_id: request.request_id };
    });
    const target = document.createElement('div');
    const component = mount(RoleManagement, { target, props: { load, change } });
    await settle();
    expect(target.textContent).toContain('Role enforcement is not active');
    rename(target, 'Release team');
    await tick();
    target
      .querySelector('form')
      ?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await settle();
    expect(change).toHaveBeenCalledOnce();
    expect(change.mock.calls[0][0]).toMatchObject({
      expected_revision: 4,
      change: { operation: 'put_role', role: { id: catalog().roles[0].id, name: 'Release team' } },
    });
    expect(load).toHaveBeenCalledTimes(2);
    expect(target.textContent).toContain('Policy revision 5');
    expect(target.textContent).toContain('Role enforcement remains inactive');
    await unmount(component);
  });

  it('keeps the exact request for uncertain retries and never shows false success', async () => {
    const load = vi.fn(async () => catalog());
    const change = vi.fn(async (_request: RoleRequest) => {
      void _request;
      throw new Error('Connection lost');
    });
    const target = document.createElement('div');
    const component = mount(RoleManagement, { target, props: { load, change } });
    await settle();
    rename(target, 'Changed');
    await tick();
    target
      .querySelector('form')
      ?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await settle();
    expect(target.querySelector('[role="alert"]')?.textContent).toContain('Connection lost');
    expect(target.textContent).not.toContain('Saved on the server');
    button(target, 'Retry same request').click();
    await settle();
    expect(change).toHaveBeenCalledTimes(2);
    expect(change.mock.calls[1][0]).toEqual(change.mock.calls[0][0]);
    button(target, 'Reload server state').click();
    await settle();
    expect(target.textContent).not.toContain('Retry same request');
    expect(target.querySelector<HTMLInputElement>('input[maxlength="64"]')?.value).toBe(
      'Developers',
    );
    await unmount(component);
  });

  it.each(['CORDS_FORBIDDEN', 'CORDS_PERMISSION_DENIED: HTTP 403 Forbidden'])(
    'surfaces %s without inventing roles or offering edits',
    async (error) => {
      const change = vi.fn();
      const target = document.createElement('div');
      const component = mount(RoleManagement, {
        target,
        props: {
          load: async () => {
            throw new Error(error);
          },
          change,
        },
      });
      await settle();
      expect(target.querySelector('[role="alert"]')?.textContent).toContain('Access restricted');
      expect(target.querySelector('details code')?.textContent).toContain(error);
      expect(
        [...target.querySelectorAll('button')].some(
          (item) => item.textContent?.trim() === 'Create role',
        ),
      ).toBe(false);
      expect(target.querySelector('form')).toBeNull();
      expect(change).not.toHaveBeenCalled();
      await unmount(component);
    },
  );
});
