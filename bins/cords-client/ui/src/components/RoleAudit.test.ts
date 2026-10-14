// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import RoleAudit from './RoleAudit.svelte';
import type { RoleAuditPage } from '../rolePolicy';
async function settle() {
  for (let i = 0; i < 30; i++) await Promise.resolve();
  await tick();
}
const page = (after: number): RoleAuditPage => ({
  version: 1,
  server_id: 'server',
  after_revision: after,
  head_revision: 2,
  entries: [
    {
      revision: after + 1,
      actor_device_id: 'device',
      target: 'account/role',
      operation: 'assign_role',
      committed_at: 1,
    },
  ],
});
describe('role change audit', () => {
  it('appends ordered pages and refreshes without duplicating records', async () => {
    const target = document.createElement('div');
    const load = vi.fn(async (after: number) => page(after));
    const component = mount(RoleAudit, { target, props: { load } });
    await settle();
    const more = [...target.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Load more'),
    );
    more?.click();
    await settle();
    expect(load.mock.calls.map((c) => c[0])).toEqual([0, 1]);
    expect(target.querySelectorAll('li')).toHaveLength(2);
    target.querySelector('button')?.click();
    await settle();
    expect(target.querySelectorAll('li')).toHaveLength(1);
    expect(target.textContent).toContain('Assigned role');
    await unmount(component);
  });
  it('shows a failed read without inventing an empty audit history', async () => {
    const target = document.createElement('div');
    const component = mount(RoleAudit, {
      target,
      props: {
        load: async () => {
          throw new Error('Forbidden');
        },
      },
    });
    await settle();
    expect(target.querySelector('[role="alert"]')?.textContent).toContain('Forbidden');
    expect(target.textContent).not.toContain('No committed role changes');
    await unmount(component);
  });
});
