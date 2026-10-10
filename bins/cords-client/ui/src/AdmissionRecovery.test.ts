// @vitest-environment jsdom
// UI-only regression: transport/security are exercised separately by the real TLS test.
import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { defaults, type Status } from './model';
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
import App from './App.svelte';

async function settle() {
  for (let i = 0; i < 30; i++) await Promise.resolve();
  await tick();
}
afterEach(() => {
  vi.useRealTimers();
  invokeMock.mockReset();
});

describe('admission and recovery presentation', () => {
  async function open(pending: boolean) {
    vi.useFakeTimers();
    Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
      configurable: true,
      value() {
        this.setAttribute('open', '');
      },
    });
    let status: Status = {
      account_id: 'account',
      device_id: 'device',
      origin: 'https://server.example',
      server_id: 'server',
      ownership_state: 'CLAIMED',
      ownership_generation: 2,
      burned: false,
      admission_state: 'active',
      join_policy: ['moderator_approval'],
      cursors: {},
    };
    invokeMock.mockImplementation(
      async (command: string, args: { action?: { kind: string } } = {}) => {
        if (command === 'list_accounts')
          return {
            accounts: [{ account_id: 'account', nickname: 'User', avatar_data: '' }],
            legacy_vault: false,
          };
        if (command === 'unlock_account') return { ...status };
        if (command === 'conversation_view')
          return {
            status: { ...status },
            identity: {
              authorization: {
                account_id: 'account',
                device_id: 'device',
                generation: 1,
                created_at: 1,
                expires_at: null,
                capabilities: [],
              },
              revoked: false,
              membership: null,
              session_expires_at: null,
            },
            preferences: defaults(),
            connected: false,
            error: null,
            messages: [],
          };
        const kind = args.action?.kind;
        if (kind === 'servers')
          return [
            {
              origin: status.origin,
              server_id: 'server',
              ownership_state: status.ownership_state,
              active: true,
              archived: false,
              remote_drop_confirmed: false,
            },
          ];
        if (kind === 'authenticate' && pending) {
          status.admission_state = 'pending';
          throw new Error('CORDS_APPROVAL_PENDING: HTTP 403 Forbidden');
        }
        if (kind === 'authenticate' || kind === 'select_server') return { ...status };
        if (kind === 'channels') return [];
        throw new Error(`Unexpected UI request: ${command}/${kind}`);
      },
    );
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(App, { target });
    await settle();
    target.querySelector<HTMLButtonElement>('.account-tile')!.click();
    await settle();
    const input = target.querySelector<HTMLInputElement>('.tile-unlock input')!;
    input.value = 'test password';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    target
      .querySelector('form.tile-unlock')!
      .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await settle();
    return {
      target,
      component,
      lockdown() {
        status = { ...status, ownership_state: 'OWNER_LOCKDOWN', ownership_generation: 3 };
      },
    };
  }
  it('explains the waiting queue without presenting the raw HTTP failure', async () => {
    const app = await open(true);
    try {
      expect(app.target.textContent).toMatch(/Join request sent\s*—\s*waiting for approval/);
      expect(app.target.textContent).toContain('Check approval');
      expect(app.target.textContent).not.toContain('HTTP 403');
      const before = invokeMock.mock.calls.filter(
        (call) => call[1]?.action?.kind === 'authenticate',
      ).length;
      await vi.advanceTimersByTimeAsync(6000);
      await settle();
      expect(
        invokeMock.mock.calls.filter((call) => call[1]?.action?.kind === 'authenticate'),
      ).toHaveLength(before);
    } finally {
      await unmount(app.component);
      app.target.remove();
    }
  });
  it('background lockdown updates the icon but recovery opens only after a click', async () => {
    const app = await open(false);
    try {
      app.lockdown();
      await vi.advanceTimersByTimeAsync(6000);
      await settle();
      expect(app.target.querySelector('dialog[aria-label="Owner recovery required"]')).toBeNull();
      const icon = app.target.querySelector<HTMLButtonElement>('.server-lockdown')!;
      expect(icon.textContent).toContain('!');
      icon.click();
      await settle();
      expect(
        app.target.querySelector('dialog[aria-label="Owner recovery required"]'),
      ).not.toBeNull();
      expect(app.target.textContent).toContain('not to ordinary members');
      app.target
        .querySelector<HTMLButtonElement>('[aria-label="Close Owner recovery required"]')!
        .click();
      await settle();
      await vi.advanceTimersByTimeAsync(6000);
      await settle();
      expect(app.target.querySelector('dialog[aria-label="Owner recovery required"]')).toBeNull();
      expect(
        invokeMock.mock.calls.filter((call) => call[1]?.action?.kind === 'recover_owner'),
      ).toHaveLength(0);
    } finally {
      await unmount(app.component);
      app.target.remove();
    }
  });
});
