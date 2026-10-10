// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { defaults, type Status } from '../model';
import CordsRail from './CordsRail.svelte';
import Admin from './Admin.svelte';

const status: Status = {
  account_id: 'account',
  device_id: 'device',
  origin: 'https://server.example',
  server_id: 'server',
  ownership_state: 'OWNER_LOCKDOWN',
  ownership_generation: 3,
  burned: false,
  admission_state: 'active',
  join_policy: ['moderator_approval'],
  cursors: {},
};

describe('server membership interface', () => {
  it('opens an app-specific context menu with distinct leave and archive actions', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const leave = vi.fn(),
      archive = vi.fn(),
      select = vi.fn();
    const component = mount(CordsRail, {
      target,
      props: {
        status,
        servers: [
          {
            origin: status.origin,
            server_id: status.server_id,
            ownership_state: 'OWNER_LOCKDOWN',
            active: true,
            archived: false,
            remote_drop_confirmed: false,
          },
        ],
        preferences: defaults(),
        section: 'server',
        select: vi.fn(),
        connect: vi.fn(),
        settings: vi.fn(),
        selectServer: select,
        leaveServer: leave,
        archiveServer: archive,
      },
    });
    const server = target.querySelector<HTMLButtonElement>('.server-blue')!;
    expect(server.getAttribute('aria-label')).toContain('Owner recovery required');
    expect(server.textContent).toContain('!');
    const event = new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: 20,
      clientY: 20,
    });
    server.dispatchEvent(event);
    await tick();
    expect(event.defaultPrevented).toBe(true);
    const items = Array.from(target.querySelectorAll<HTMLButtonElement>('[role="menuitem"]'));
    expect(items.map((item) => item.textContent)).toEqual([
      'Open server',
      'Leave server…',
      'Archive server…',
    ]);
    items[1].click();
    await tick();
    expect(leave).toHaveBeenCalledWith('server');
    expect(archive).not.toHaveBeenCalled();
    expect(select).not.toHaveBeenCalled();
    server.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await tick();
    target.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')[2].click();
    await tick();
    expect(archive).toHaveBeenCalledWith('server');
    await unmount(component);
    target.remove();
  });

  it('shows a self-declared nickname, avatar resolution information and full fingerprint', async () => {
    Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
      configurable: true,
      value() {
        this.setAttribute('open', '');
      },
    });
    const target = document.createElement('div');
    document.body.append(target);
    const fingerprint = 'FULL-ROOT-IDENTITY-FINGERPRINT';
    const component = mount(Admin, {
      target,
      props: {
        status: { ...status, ownership_state: 'CLAIMED' },
        channels: [],
        contacts: [],
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
          membership: { capabilities: ['server.manage'], status: 'active', issued_at: 1 },
          session_expires_at: null,
        },
        membershipRequests: [
          {
            account_id: fingerprint,
            device_id: 'requesting-device',
            requested_at: 1,
            status: 'pending',
            identity_burned: false,
            user_card: {
              value: {
                version: 1,
                account_id: fingerprint,
                device_id: 'requesting-device',
                nickname: 'Second user',
                issued_at: 1,
                avatar: {
                  data: 'data:image/png;base64,aGVsbG8=',
                  shape: 'square',
                  x: 25,
                  y: 75,
                  zoom_milli: 1500,
                },
              },
            },
          },
        ],
        close: vi.fn(),
        create: vi.fn(),
        select: vi.fn(),
        decideMembership: vi.fn(),
      },
    });
    Array.from(target.querySelectorAll<HTMLButtonElement>('.settings-menu-group button'))
      .find((button) => button.textContent === 'Members')!
      .click();
    await tick();
    expect(target.textContent).toContain('Second user');
    expect(target.textContent).toContain(fingerprint);
    expect(target.textContent).toContain('Self-declared');
    const avatar = target.querySelector<HTMLImageElement>('.membership-request-card img');
    expect(avatar?.getAttribute('src')).toContain('data:image/png');
    expect(avatar?.getAttribute('style')).toContain('25% 75%');
    await unmount(component);
    target.remove();
  });
});
