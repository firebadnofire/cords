import { describe, expect, it } from 'vitest';
import {
  availability,
  canManage,
  preferences,
  dmEntry,
  picture,
  selectedChannel,
  type Channel,
  type Identity,
} from './model';
describe('persisted presentation boundary', () => {
  it('never renders arbitrary stored image URLs or CSS colors', () => {
    const p = preferences({
      avatar: { data: 'https://tracker.example/a' },
      jewelColor: 'url(https://tracker.example)',
      theme: 'invented',
    });
    expect(p.avatar.data).toBe('');
    expect(p.jewelColor).toBe('#477da3');
    expect(p.theme).toBe('dark');
  });
  it('has exactly one DM entry for each Jewel action', () => {
    expect(dmEntry('dms')).toBe('jewel');
    expect(dmEntry('none')).toBe('separate');
    expect(dmEntry('settings')).toBe('separate');
  });
  it('bounds malformed crop preferences', () => {
    expect(picture({ x: NaN, y: 900, zoom: -4 })).toMatchObject({ x: 50, y: 100, zoom: 1 });
  });
  it('classifies real and future controls explicitly', () => {
    expect(availability(true)).toBe('available');
    expect(availability(false)).toBe('development');
  });
  it('maps a real route to its donor-derived conversation surface', () => {
    const channel = { channel_id: 'route-1', name: 'general' } as Channel;
    expect(selectedChannel([channel], 'route-1')).toBe(channel);
    expect(selectedChannel([channel], 'missing')).toBeUndefined();
  });
  it('allows channel creators and authenticated server owners to manage a channel', () => {
    const channel = { channel_id: 'route-1', creator_device_id: 'creator' } as Channel;
    const status = { device_id: 'other' } as import('./model').Status;
    const owner = {
      membership: { capabilities: ['server.manage', 'channel.manage'] },
    } as Identity;
    expect(canManage(channel, { device_id: 'creator' } as import('./model').Status, null)).toBe(
      true,
    );
    expect(canManage(channel, status, owner)).toBe(true);
    expect(canManage(channel, status, null)).toBe(false);
  });
});
