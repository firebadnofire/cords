// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import MemberRoles from './MemberRoles.svelte';
import type { AccountRoles, RoleRequest } from '../rolePolicy';

const snapshot = (): AccountRoles => ({
  account_id: 'account',
  role_ids: [],
  catalog: {
    version: 1,
    server_id: 'server',
    revision: 5,
    active: false,
    roles: [
      {
        id: 'builtin:default',
        name: 'default',
        color: '#4488aa',
        description: '',
        priority: 0,
        permissions: [],
        mentionable: false,
      },
      {
        id: 'team-id',
        name: 'Team',
        color: '#4488aa',
        description: '',
        priority: 1,
        permissions: [],
        mentionable: false,
      },
    ],
  },
});
async function settle() {
  for (let i = 0; i < 30; i++) await Promise.resolve();
  await tick();
}
function click(target: HTMLElement, label: string) {
  const button = [...target.querySelectorAll('button')].find(
    (b) => b.textContent?.trim() === label,
  );
  if (!button) throw new Error(`Missing ${label}`);
  button.click();
}
describe('account role assignments', () => {
  it('uses account identity and committed revision, then reloads assignment state', async () => {
    let state = snapshot();
    const change = vi.fn(async (request: RoleRequest) => {
      state = { ...state, catalog: { ...state.catalog, revision: 6 }, role_ids: ['team-id'] };
      return { revision: 6, request_id: request.request_id };
    });
    const target = document.createElement('div');
    const component = mount(MemberRoles, {
      target,
      props: { account: 'account', load: async () => state, change },
    });
    await settle();
    expect(target.textContent).toContain('Implicit for admitted accounts');
    click(target, 'Assign $Team');
    await settle();
    expect(change.mock.calls[0][0]).toMatchObject({
      expected_revision: 5,
      change: { operation: 'assign_role', account_id: 'account', role_id: 'team-id' },
    });
    expect(target.textContent).toContain('Remove $Team');
    expect(target.textContent).toContain('Role enforcement remains inactive');
    await unmount(component);
  });
  it('retries the identical request without claiming a failed assignment succeeded', async () => {
    const change = vi.fn(async (_request: RoleRequest) => {
      void _request;
      throw new Error('Connection lost');
    });
    const target = document.createElement('div');
    const component = mount(MemberRoles, {
      target,
      props: { account: 'account', load: async () => snapshot(), change },
    });
    await settle();
    click(target, 'Assign $Team');
    await settle();
    click(target, 'Retry assignment');
    await settle();
    expect(change.mock.calls[1][0]).toEqual(change.mock.calls[0][0]);
    expect(target.textContent).not.toContain('Assignment saved');
    expect(target.textContent).not.toContain('Remove $Team');
    await unmount(component);
  });
  it('rejects a snapshot for another account without exposing assignment controls', async () => {
    const target = document.createElement('div');
    const component = mount(MemberRoles, {
      target,
      props: { account: 'other', load: async () => snapshot(), change: vi.fn() },
    });
    await settle();
    expect(target.querySelector('[role="alert"]')?.textContent).toContain('does not match');
    expect(target.textContent).not.toContain('Assign $Team');
    await unmount(component);
  });
});
