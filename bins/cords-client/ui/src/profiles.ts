import { invoke } from '@tauri-apps/api/core';
import { writable, get } from 'svelte/store';

export type ProfileAvatar = {
  sha256: string;
  bytes: number;
  width: number;
  height: number;
  shape: 'circle' | 'square';
};
export type PublicProfile = { display_name: string; avatar: ProfileAvatar | null };
export type ProfileView = {
  candidates?: { id: string; profile: PublicProfile }[];
  state: {
    heads: string[];
    pending: string[];
    agreed: PublicProfile | null;
    conflicts: ('display_name' | 'avatar')[];
  };
  image: string | null;
  pending_publication: string | null;
};
export const publicProfiles = writable<Record<string, ProfileView>>({});
export const avatarMotion = writable({ disabled: false, reduced: false });
let scope = '';
let generation = 0;
const pending = new Map<string, Promise<ProfileView>>();
const refreshed = new Map<string, number>();
export function profileScope(next: string) {
  if (next === scope) return;
  scope = next;
  generation++;
  publicProfiles.set({});
  pending.clear();
  refreshed.clear();
  cancelAvatarProcessing();
}
let cancelProcessing = () => {};
export function registerAvatarProcessing(cancel: () => void) {
  cancelProcessing = cancel;
}
export function cancelAvatarProcessing() {
  cancelProcessing();
}

export function rememberProfile(account: string, view: ProfileView) {
  publicProfiles.update((profiles) => {
    const next = { ...profiles, [account]: view };
    // Compressed media cache is separate from native signed history. Bound the
    // WebView's strings to 32 MiB; at most 16 MiB decoded posters live in players.
    let bytes = Object.values(next).reduce(
      (sum, profile) => sum + (profile.image?.length ?? 0) * 2,
      0,
    );
    for (const key of Object.keys(next)) {
      if (bytes <= 32 * 1024 * 1024) break;
      if (key !== account) {
        bytes -= (next[key].image?.length ?? 0) * 2;
        delete next[key];
      }
    }
    return next;
  });
  refreshed.set(account, Date.now());
}
export function loadProfile(account: string, force = false): Promise<ProfileView> {
  const cached = get(publicProfiles)[account];
  if (!scope || !/^[A-Za-z0-9_-]{43}$/.test(account))
    return Promise.reject(new Error('Unlock and select a server to load profiles.'));
  if (!force && cached && Date.now() - (refreshed.get(account) ?? 0) < 15000)
    return Promise.resolve(cached);
  const running = pending.get(account);
  if (running) return running;
  const version = generation;
  const request = invoke<ProfileView>('conversation_action', {
    action: { kind: 'account_profile', account },
  })
    .then((view) => {
      if (version !== generation)
        throw new Error('Account or server changed during profile retrieval.');
      rememberProfile(account, view);
      return view;
    })
    .finally(() => {
      if (version === generation) pending.delete(account);
    });
  pending.set(account, request);
  return request;
}
