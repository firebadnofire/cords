// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import MembersPanel from './MembersPanel.svelte';
import type { AccountDirectoryPage, Contact } from '../model';
const device = (id: string, nickname: string): Contact => ({
  authorization: {
    value: {
      account_id: 'account',
      device_id: id,
      generation: 1,
      created_at: 1,
      expires_at: null,
      capabilities: [],
    },
  },
  user_card: {
    value: {
      version: 1,
      account_id: 'account',
      device_id: id,
      nickname,
      avatar: null,
      issued_at: 1,
    },
  },
});
const page = (names = ['Alice', 'Alice']): AccountDirectoryPage => ({
  version: 1,
  server_id: 'server',
  next_after: null,
  accounts: [
    { account_id: 'account', devices: names.map((name, i) => device(`device-${i}`, name)) },
  ],
});
async function settle() {
  for (let i = 0; i < 15; i++) await Promise.resolve();
  await tick();
}
describe('server members', () => {
  it('shows one account for multiple devices and searches it without inventing presence', async () => {
    const target = document.createElement('div');
    const app = mount(MembersPanel, {
      target,
      props: { serverId: 'server', load: vi.fn(async () => page()), close: vi.fn() },
    });
    await settle();
    expect(target.querySelectorAll('.member-list-row')).toHaveLength(1);
    expect(target.textContent).toContain('Alice');
    expect(target.textContent).not.toContain('Online');
    target.querySelector<HTMLButtonElement>('.member-list-row')!.click();
    await tick();
    expect(target.textContent).toContain('Devices (2)');
    const search = target.querySelector('input')!;
    search.value = 'missing';
    search.dispatchEvent(new Event('input'));
    await tick();
    expect(target.textContent).toContain('No matching loaded members.');
    await unmount(app);
  });
  it('does not choose a conflicting device card as the account profile', async () => {
    const target = document.createElement('div');
    const app = mount(MembersPanel, {
      target,
      props: {
        serverId: 'server',
        load: vi.fn(async () => page(['Alice', 'Bob'])),
        close: vi.fn(),
      },
    });
    await settle();
    expect(target.textContent).toContain('Profile needs review');
    expect(target.querySelector('.member-list-row')?.textContent).not.toContain('Alice');
    await unmount(app);
  });
  it('rejects another server response and permits explicit retry', async () => {
    const target = document.createElement('div');
    const load = vi
      .fn()
      .mockResolvedValueOnce({ ...page(), server_id: 'other' })
      .mockResolvedValueOnce(page());
    const app = mount(MembersPanel, {
      target,
      props: { serverId: 'server', load, close: vi.fn() },
    });
    await settle();
    expect(target.querySelector('[role="alert"]')?.textContent).toContain('Server changed');
    target.querySelector<HTMLButtonElement>('[aria-label="Refresh members"]')!.click();
    await settle();
    expect(target.querySelector('[role="alert"]')).toBeNull();
    expect(target.textContent).toContain('Alice');
    await unmount(app);
  });
  it('clears stale members after authorization is lost during refresh', async () => {
    const target = document.createElement('div');
    const load = vi.fn().mockResolvedValueOnce(page()).mockRejectedValueOnce(new Error('HTTP 403'));
    const app = mount(MembersPanel, {
      target,
      props: { serverId: 'server', load, close: vi.fn() },
    });
    await settle();
    expect(target.querySelectorAll('.member-list-row')).toHaveLength(1);
    target.querySelector<HTMLButtonElement>('[aria-label="Refresh members"]')!.click();
    await settle();
    expect(target.querySelectorAll('.member-list-row')).toHaveLength(0);
    expect(target.querySelector('[role="alert"]')?.textContent).toContain('HTTP 403');
    expect(target.textContent).not.toContain('server needs an update');
    await unmount(app);
  });
});
