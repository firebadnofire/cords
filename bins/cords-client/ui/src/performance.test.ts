// @vitest-environment jsdom
import { mount, tick, unmount } from 'svelte';
import { describe, expect, it } from 'vitest';
import ConversationView from './components/ConversationView.svelte';
import { defaults, type Channel, type Message, type Status } from './model';

describe('representative conversation performance guard', () => {
  it('bounds the initial DOM while rendering a long conversation', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const messages: Message[] = Array.from({ length: 1_200 }, (_, index) => ({
      message_id: `message-${index}`,
      sender_device_id: index % 2 ? 'device-a' : 'device-b',
      body: `Representative encrypted message ${index}`,
      client_timestamp: 1_700_000_000 + index,
    }));
    const channel = {
      channel_id: 'long-route',
      name: 'performance',
      creator_device_id: 'device-a',
      epoch: 7,
      members: [],
    } satisfies Channel;
    const status = {
      account_id: 'account',
      device_id: 'device-a',
      origin: 'https://server.example:4848',
      server_id: 'server',
      cursors: {},
    } satisfies Status;
    const runtime = globalThis as typeof globalThis & {
      process?: { memoryUsage: () => { heapUsed: number } };
    };
    const heapBefore = runtime.process?.memoryUsage().heapUsed ?? 0;
    const started = performance.now();
    const component = mount(ConversationView, {
      target,
      props: {
        section: 'server',
        channel,
        messages,
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
    await Promise.resolve();
    const elapsed = performance.now() - started;
    const rendered = target.querySelectorAll('article.cords-message').length;
    const nodes = target.querySelectorAll('*').length;
    const heapDelta = (runtime.process?.memoryUsage().heapUsed ?? heapBefore) - heapBefore;
    expect(rendered).toBe(500);
    expect(target.textContent).toContain('Show 500 earlier messages');
    expect(nodes).toBeLessThan(10_000);
    const expansionStarted = performance.now();
    (target.querySelector('button.load-earlier') as HTMLButtonElement).click();
    await tick();
    const expansionElapsed = performance.now() - expansionStarted;
    expect(target.querySelectorAll('article.cords-message')).toHaveLength(1_000);
    console.info(
      `conversation-performance initial_render_ms=${elapsed.toFixed(1)} initial_messages=${rendered} initial_dom_nodes=${nodes} heap_delta_mb=${(heapDelta / 1024 / 1024).toFixed(1)} expand_500_ms=${expansionElapsed.toFixed(1)}`,
    );
    await unmount(component);
    target.remove();
  });
});
