// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { defaults, type Channel, type Status } from '../model';
import ConfidentialitySelect from './ConfidentialitySelect.svelte';
import ConversationView from './ConversationView.svelte';

const status: Status = {
  account_id: 'account',
  device_id: 'device',
  origin: 'https://example.com',
  server_id: 'server',
  ownership_state: 'CLAIMED',
  ownership_generation: 1,
  burned: false,
  admission_state: 'active',
  join_policy: ['public'],
  cursors: {},
};

describe('channel confidentiality', () => {
  it('defaults to encrypted and explains the permanent public choice', async () => {
    const target = document.createElement('div');
    const component = mount(ConfidentialitySelect, { target });
    const select = target.querySelector('select')!;
    expect(select.value).toBe('encrypted');
    select.value = 'public';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    await tick();
    expect(target.textContent).toContain('not end-to-end encrypted');
    expect(target.textContent).toContain('cryptographically verified');
    expect(target.textContent).toContain('permanent');
    await unmount(component);
  });

  it.each(['acknowledgement', 'archive'])(
    'prevents composing into a gated %s channel',
    async (gate) => {
      const target = document.createElement('div');
      const channel: Channel = {
        channel_id: 'new-id',
        name: 'same name',
        creator_device_id: 'device',
        epoch: 0,
        members: [],
        confidentiality_mode: 'public',
        requires_public_acknowledgement: gate === 'acknowledgement',
        locally_archived: gate === 'archive',
      };
      const component = mount(ConversationView, {
        target,
        props: {
          section: 'server',
          channel,
          messages: [],
          status,
          preferences: defaults(),
          query: '',
          body: 'test',
          busy: false,
          revoked: false,
          setBody: vi.fn(),
          send: vi.fn(),
        },
      });
      expect(target.querySelector<HTMLInputElement>('#message-draft')!.disabled).toBe(true);
      expect(target.querySelector<HTMLButtonElement>('button[type="submit"]')!.disabled).toBe(true);
      expect(target.textContent).toContain('no end-to-end encryption');
      expect(target.textContent?.replace(/\s+/g, ' ')).toContain('read, copy, and retain');
      await unmount(component);
    },
  );
});
