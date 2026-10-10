// @vitest-environment jsdom
import { mount, unmount } from 'svelte';
import { describe, expect, it } from 'vitest';
import { defaults, type Channel, type Status } from '../model';
import ConversationView from './ConversationView.svelte';

describe('conversation composer', () => {
  it('opts out of saved input history', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const channel = {
      channel_id: 'channel-id',
      name: 'general',
      creator_device_id: 'device-id',
      epoch: 1,
      members: [],
    } satisfies Channel;
    const status = {
      account_id: 'account-id',
      device_id: 'device-id',
      origin: 'https://server.example:4848',
      server_id: 'server-id',
      ownership_state: 'CLAIMED',
      ownership_generation: 2,
      burned: false,
      join_policy: ['moderator_approval'],
      cursors: {},
    } satisfies Status;
    const component = mount(ConversationView, {
      target,
      props: {
        section: 'server',
        channel,
        messages: [],
        status,
        preferences: defaults(),
        query: '',
        body: '',
        busy: false,
        revoked: false,
        setBody: () => undefined,
        send: () => undefined,
      },
    });

    const form = target.querySelector('form.cords-composer');
    const input = target.querySelector<HTMLInputElement>('#message-draft');
    expect(form?.getAttribute('autocomplete')).toBe('off');
    expect(input?.getAttribute('autocomplete')).toBe('off');

    await unmount(component);
    target.remove();
  });
});
