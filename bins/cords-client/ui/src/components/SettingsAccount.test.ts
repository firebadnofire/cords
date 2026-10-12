// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import Settings from './Settings.svelte';
import { defaults } from '../model';

beforeAll(() => {
  // jsdom lacks native dialog presentation; test the Settings behavior independently.
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
});

async function setup() {
  const target = document.createElement('div');
  const changePassword = vi.fn().mockResolvedValue(undefined);
  const openArchives = vi.fn();
  const component = mount(Settings, {
    target,
    props: {
      value: defaults(),
      identity: null,
      status: null,
      servers: [
        {
          server_id: 'old',
          origin: 'https://old.example',
          archived: true,
          remote_drop_confirmed: true,
          ownership_state: 'CLAIMED',
          active: false,
        },
      ],
      close: vi.fn(),
      save: vi.fn(),
      revoke: vi.fn(),
      lockNow: vi.fn(),
      removeServer: vi.fn(),
      archiveServer: vi.fn(),
      burnIdentity: vi.fn(),
      designateSuccessor: vi.fn(),
      acceptSuccessor: vi.fn(),
      openArchives,
      changePassword,
      assessPassword: vi.fn().mockResolvedValue({ weak: false, reasons: [] }),
    },
  });
  await tick();
  async function page(name: string) {
    const button = [...target.querySelectorAll('button')].find(
      (b) => b.textContent?.trim() === name,
    );
    expect(button).toBeDefined();
    button!.click();
    await tick();
  }
  async function fields(current: string, next: string, confirmation: string) {
    const inputs = target.querySelectorAll<HTMLInputElement>('input[type="password"]');
    for (const [i, value] of [current, next, confirmation].entries()) {
      inputs[i].value = value;
      inputs[i].dispatchEvent(new Event('input', { bubbles: true }));
    }
    await tick();
  }
  async function submit() {
    target
      .querySelector('form')!
      .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    for (let i = 0; i < 20; i++) await Promise.resolve();
    await tick();
  }
  return { target, component, changePassword, openArchives, page, fields, submit };
}

describe('account settings', () => {
  it('opens partial archives from Local history and distinguishes server bookmarks', async () => {
    const ui = await setup();
    await ui.page('Local history');
    expect(ui.target.textContent).toContain('https://old.example');
    expect(ui.target.textContent).toContain('not complete server histories');
    await ui.page('Browse retained channel archives');
    expect(ui.openArchives).toHaveBeenCalledOnce();
    await unmount(ui.component);
  });
  it('requires matching confirmation and clears password fields after success', async () => {
    const ui = await setup();
    await ui.page('Security');
    await ui.fields('current password', 'different new password', 'mismatch');
    await ui.submit();
    expect(ui.changePassword).not.toHaveBeenCalled();
    expect(ui.target.textContent).toContain('Passwords do not match');
    await ui.fields('current password', 'different new password', 'different new password');
    await ui.submit();
    expect(ui.changePassword).toHaveBeenCalledWith(
      'current password',
      'different new password',
      false,
    );
    expect(ui.target.textContent).toContain('Password changed.');
    expect(
      [...ui.target.querySelectorAll<HTMLInputElement>('input[type="password"]')].every(
        (input) => input.value === '',
      ),
    ).toBe(true);
    await unmount(ui.component);
  });
  it('surfaces native failures and clears secret fields without claiming success', async () => {
    const ui = await setup();
    ui.changePassword.mockRejectedValue('current password was not accepted');
    await ui.page('Security');
    await ui.fields('incorrect', 'different new password', 'different new password');
    await ui.submit();
    expect(ui.target.textContent).toContain('current password was not accepted');
    expect(ui.target.textContent).not.toContain('Password changed.');
    expect(
      [...ui.target.querySelectorAll<HTMLInputElement>('input[type="password"]')].every(
        (input) => input.value === '',
      ),
    ).toBe(true);
    await unmount(ui.component);
  });
});
