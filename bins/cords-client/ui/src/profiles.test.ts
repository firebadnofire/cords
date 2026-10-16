import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
import { loadProfile, profileScope, publicProfiles, type ProfileView } from './profiles';
const account = 'A'.repeat(43);
const view: ProfileView = {
  state: { heads: [], pending: [], agreed: { display_name: 'Alice', avatar: null }, conflicts: [] },
  image: null,
  pending_publication: null,
};
beforeEach(() => {
  profileScope('');
  profileScope('first-account/server');
  invokeMock.mockReset();
});
describe('verified account profile presentation', () => {
  it('requests the account profile once for concurrent appearances', async () => {
    invokeMock.mockResolvedValue(view);
    await Promise.all([loadProfile(account), loadProfile(account)]);
    expect(invokeMock).toHaveBeenCalledOnce();
    expect(invokeMock).toHaveBeenCalledWith('conversation_action', {
      action: { kind: 'account_profile', account },
    });
    expect(get(publicProfiles)[account].state.agreed?.display_name).toBe('Alice');
  });
  it('drops an old account response after lock or account/server switch', async () => {
    let finish!: (value: ProfileView) => void;
    invokeMock.mockImplementation(
      () =>
        new Promise<ProfileView>((resolve) => {
          finish = resolve;
        }),
    );
    const pending = loadProfile(account);
    profileScope('other-account/server');
    finish(view);
    await expect(pending).rejects.toThrow('Account or server changed');
    expect(get(publicProfiles)).toEqual({});
  });
  it('surfaces synchronization errors without fabricating a profile', async () => {
    invokeMock.mockRejectedValue(new Error('This server needs an update'));
    await expect(loadProfile(account)).rejects.toThrow('server needs an update');
    expect(get(publicProfiles)).toEqual({});
  });
});
