// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import Settings from './Settings.svelte';
import { defaults, type Status } from '../model';

beforeAll(() => {
  // jsdom lacks native dialog presentation; test the Settings behavior independently.
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
});

async function setup(status: Status | null = null) {
  const target = document.createElement('div');
  const changePassword = vi.fn().mockResolvedValue(undefined);
  const openArchives = vi.fn();
  const save = vi.fn().mockResolvedValue(undefined);
  const close = vi.fn();
  const component = mount(Settings, {
    target,
    props: {
      value: defaults(),
      identity: null,
      status,
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
      close,
      save,
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
  return { target, component, changePassword, openArchives, save, close, page, fields, submit };
}

describe('account settings', () => {
  it('preserves unsaved edits on Escape and closes only after explicit discard', async () => {
    const ui = await setup();
    await ui.page('Appearance');
    await ui.page('Midnight');
    ui.target.querySelector('dialog')!.dispatchEvent(new Event('cancel', { cancelable: true }));
    await tick();
    expect(ui.close).not.toHaveBeenCalled();
    expect(ui.target.textContent).toContain('Keep your changes?');
    await ui.page('Keep editing');
    expect(ui.target.textContent).not.toContain('Keep your changes?');
    expect(ui.target.textContent).toContain('Unsaved changes');
    ui.target.querySelector('dialog')!.dispatchEvent(new Event('cancel', { cancelable: true }));
    await tick();
    await ui.page('Discard and close');
    expect(ui.close).toHaveBeenCalledOnce();
    expect(ui.save).not.toHaveBeenCalled();
    await unmount(ui.component);
  });

  it('retains a failed preference save and enables retry without false success', async () => {
    const ui = await setup({ server_id: 'server' } as Status);
    await ui.page('Appearance');
    const saveButton = () =>
      [...ui.target.querySelectorAll('button')].find(
        (button) => button.textContent?.trim() === 'Save local preferences',
      )!;
    expect(saveButton().disabled).toBe(true);
    await ui.page('Midnight');
    ui.save.mockRejectedValueOnce(new Error('Local storage unavailable'));
    await ui.page('Save local preferences');
    for (let i = 0; i < 20; i++) await Promise.resolve();
    await tick();
    expect(ui.target.textContent).toContain('Local storage unavailable');
    expect(ui.target.textContent).toContain('Unsaved changes');
    expect(ui.target.textContent).not.toContain('Saved on this installation');
    await ui.page('Save local preferences');
    for (let i = 0; i < 20; i++) await Promise.resolve();
    await tick();
    expect(ui.save).toHaveBeenLastCalledWith(expect.objectContaining({ theme: 'midnight' }));
    expect(ui.target.textContent).toContain('Saved on this installation');
    expect(saveButton().disabled).toBe(true);
    await unmount(ui.component);
  });
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
