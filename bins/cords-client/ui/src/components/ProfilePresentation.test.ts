// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { defaults, type Contact, type Status } from '../model';
import ConversationView from './ConversationView.svelte';
import ConversationDetails from './ConversationDetails.svelte';
import AccountPicker from './AccountPicker.svelte';
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('../images', () => ({ importImage: vi.fn(async () => 'data:image/png;base64,aGVsbG8=') }));
const status: Status = {
  account_id: 'fire',
  device_id: 'fire-device',
  origin: 'https://example.com',
  server_id: 'server',
  ownership_state: 'CLAIMED',
  ownership_generation: 1,
  burned: false,
  admission_state: 'active',
  join_policy: ['public'],
  cursors: {},
};
const contact: Contact = {
  authorization: {
    value: {
      account_id: 'arbiter',
      device_id: 'arbiter-device',
      generation: 1,
      created_at: 1,
      expires_at: null,
      capabilities: [],
    },
  },
  user_card: {
    value: {
      version: 1,
      account_id: 'arbiter',
      device_id: 'arbiter-device',
      nickname: 'Arbiter',
      issued_at: 1,
      avatar: {
        url: 'https://images.example/arbiter.webp',
        shape: 'square',
        x: 25,
        y: 75,
        zoom_milli: 1500,
      },
    },
  },
};
const channel = {
  channel_id: 'channel',
  name: 'Chat',
  creator_device_id: status.device_id,
  epoch: 0,
  members: [],
};
async function settle() {
  for (let i = 0; i < 20; i++) await Promise.resolve();
  await tick();
}
describe('verified profile presentation', () => {
  it('shows another device’s nickname and resolved cropped picture in public messages', async () => {
    invokeMock.mockResolvedValue(btoa('image'));
    const target = document.createElement('div');
    const component = mount(ConversationView, {
      target,
      props: {
        section: 'server',
        channel: { ...channel, confidentiality_mode: 'public' },
        messages: [
          {
            message_id: 'message',
            sender_device_id: 'arbiter-device',
            body: 'Hello',
            client_timestamp: 1,
          },
        ],
        contacts: [contact],
        status,
        preferences: defaults(),
        query: 'Arbiter',
        body: '',
        busy: false,
        revoked: false,
        setBody: vi.fn(),
        send: vi.fn(),
      },
    });
    await settle();
    expect(target.querySelector('.message-meta strong')?.textContent).toBe('Arbiter');
    expect(target.querySelector('.message-meta strong')?.getAttribute('title')).toBe(
      'arbiter-device',
    );
    expect(target.querySelector('.cords-message img')?.getAttribute('style')).toContain('25% 75%');
    expect(invokeMock).toHaveBeenCalledWith('load_image_url', {
      url: 'https://images.example/arbiter.webp',
    });
    await unmount(component);
  });
  it('offers named members with device identity and key-package guidance', async () => {
    const target = document.createElement('div');
    const add = vi.fn();
    const component = mount(ConversationDetails, {
      target,
      props: {
        channel,
        status,
        contacts: [contact],
        manager: true,
        busy: false,
        close: vi.fn(),
        removeDevice: vi.fn(),
        addDevice: add,
        publish: vi.fn(),
      },
    });
    const select = target.querySelector('select')!;
    expect(select.textContent).toContain('Arbiter');
    expect(target.textContent?.replace(/\s+/g, ' ')).toContain('publish a KeyPackage first');
    select.value = 'arbiter-device';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    await tick();
    target
      .querySelector('form')!
      .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(add).toHaveBeenCalledWith('arbiter-device');
    await unmount(component);
  });
  it('uses a neutral icon for hidden locked nicknames', async () => {
    const target = document.createElement('div');
    const component = mount(AccountPicker, {
      target,
      props: {
        accounts: [
          {
            account_id: 'fire',
            nickname: 'Account 2',
            avatar_data: '',
            hide_nickname_on_lock: true,
          },
        ],
        unlock: vi.fn(),
        create: vi.fn(),
        migrate: vi.fn(),
        assess: vi.fn(),
      },
    });
    expect(target.querySelector('.picker-avatar b')).toBeNull();
    expect(target.querySelector('.picker-avatar svg')).not.toBeNull();
    expect(target.textContent).toContain('Account 2');
    await unmount(component);
  });
});
