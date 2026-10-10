export type Authorization = {
  account_id: string;
  device_id: string;
  generation: number;
  created_at: number;
  expires_at: number | null;
  capabilities: string[];
};
export type UserCard = {
  version: number;
  account_id: string;
  device_id: string;
  nickname: string;
  avatar: {
    url?: string;
    data?: string;
    shape: string;
    x: number;
    y: number;
    zoom_milli: number;
  } | null;
  issued_at: number;
};
export type Contact = { authorization: { value: Authorization }; user_card?: { value: UserCard } };
export type ConfidentialityMode = 'encrypted' | 'public';
export type Channel = {
  confidentiality_mode?: ConfidentialityMode;
  identity?: {
    value: {
      server_id: string;
      channel_id: string;
      name: string;
      confidentiality_mode: ConfidentialityMode;
    };
  };
  transition?: {
    value: {
      retired: boolean;
      succession: {
        value: {
          predecessor: {
            value: { channel_id: string; name: string; confidentiality_mode: ConfidentialityMode };
          };
          successor_channel_id: string;
          name: string;
          confidentiality_mode: ConfidentialityMode;
        };
      } | null;
    };
  };
  requires_public_acknowledgement?: boolean;
  locally_archived?: boolean;
  channel_id: string;
  name: string;
  creator_device_id: string;
  epoch: number;
  members: Contact[];
};
export type MembershipRequest = {
  account_id: string;
  device_id: string;
  requested_at: number;
  status: string;
  identity_burned: boolean;
  user_card: { value: UserCard } | null;
};
export type ServerListing = {
  origin: string;
  server_id: string;
  ownership_state: string;
  active: boolean;
  archived: boolean;
  remote_drop_confirmed: boolean;
};
export type Status = {
  account_id: string;
  device_id: string;
  origin: string;
  server_id: string;
  ownership_state: string;
  ownership_generation: number;
  burned: boolean;
  admission_state: string;
  join_policy: string[];
  cursors: Record<string, number>;
};
export type Message = {
  message_id: string;
  sender_device_id: string;
  body: string;
  client_timestamp: number;
};
export type Identity = {
  authorization: Authorization;
  revoked: boolean;
  membership: { capabilities: string[]; status: string; issued_at: number } | null;
  session_expires_at: number | null;
  burn_deliveries?: { origin: string; last_error: string | null; next_retry_at: number }[];
};
export type View = {
  status: Status;
  identity: Identity;
  preferences: unknown;
  connected: boolean;
  error: string | null;
  messages: Message[];
};
export type Picture = {
  url: string;
  data: string;
  shape: 'circle' | 'square';
  x: number;
  y: number;
  zoom: number;
};
export type Preferences = {
  version: 1;
  displayName: string;
  theme: 'dark' | 'light' | 'midnight';
  compact: boolean;
  reducedMotion: boolean;
  jewelText: string;
  jewelColor: string;
  jewelAction: 'dms' | 'settings' | 'none';
  avatar: Picture;
  jewel: Picture;
  genericize: boolean;
  hideNicknameOnLock: boolean;
  autoLockMinutes: 1 | 5 | 15 | 30 | 60 | null;
  lockOnOsLock: boolean;
  lockOnSuspend: boolean;
};
export const blankPicture = (): Picture => ({
  url: '',
  data: '',
  shape: 'circle',
  x: 50,
  y: 50,
  zoom: 1,
});
export const defaults = (): Preferences => ({
  version: 1,
  displayName: 'You',
  theme: 'dark',
  compact: false,
  reducedMotion: false,
  jewelText: 'C',
  jewelColor: '#477da3',
  jewelAction: 'dms',
  avatar: blankPicture(),
  jewel: blankPicture(),
  genericize: false,
  hideNicknameOnLock: false,
  autoLockMinutes: 15,
  lockOnOsLock: true,
  lockOnSuspend: true,
});
const bounded = (n: unknown, min: number, max: number, fallback: number) =>
  typeof n === 'number' && Number.isFinite(n) ? Math.max(min, Math.min(max, n)) : fallback;
export function picture(value: unknown): Picture {
  const p = (value && typeof value === 'object' ? value : {}) as Partial<Picture>;
  return {
    url:
      typeof p.url === 'string' && /^https:\/\/[^\s@]+$/i.test(p.url) && p.url.length <= 2048
        ? p.url
        : '',
    data:
      typeof p.data === 'string' &&
      /^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(p.data) &&
      p.data.length < 900_000
        ? p.data
        : '',
    shape: p.shape === 'square' ? 'square' : 'circle',
    x: bounded(p.x, 0, 100, 50),
    y: bounded(p.y, 0, 100, 50),
    zoom: bounded(p.zoom, 1, 4, 1),
  };
}
export function preferences(value: unknown): Preferences {
  const p = (value && typeof value === 'object' ? value : {}) as Partial<Preferences>;
  const autoLockMinutes: Preferences['autoLockMinutes'] =
    p.autoLockMinutes === null || [1, 5, 15, 30, 60].includes(p.autoLockMinutes as number)
      ? (p.autoLockMinutes ?? null)
      : 15;
  return {
    ...defaults(),
    displayName: typeof p.displayName === 'string' ? p.displayName.slice(0, 100) : 'You',
    theme: p.theme === 'light' || p.theme === 'midnight' ? p.theme : 'dark',
    compact: p.compact === true,
    reducedMotion: p.reducedMotion === true,
    jewelText: typeof p.jewelText === 'string' ? p.jewelText.slice(0, 3) : 'C',
    jewelColor:
      typeof p.jewelColor === 'string' && /^#[0-9a-f]{6}$/i.test(p.jewelColor)
        ? p.jewelColor
        : '#477da3',
    jewelAction: p.jewelAction === 'none' || p.jewelAction === 'settings' ? p.jewelAction : 'dms',
    avatar: picture(p.avatar),
    jewel: picture(p.jewel),
    genericize: p.genericize === true,
    hideNicknameOnLock: p.hideNicknameOnLock === true,
    autoLockMinutes,
    lockOnOsLock: p.lockOnOsLock !== false,
    lockOnSuspend: p.lockOnSuspend !== false,
  };
}
export function dmEntry(action: Preferences['jewelAction']) {
  return action === 'dms' ? 'jewel' : 'separate';
}
export function canManage(
  channel: Channel | undefined,
  status: Status | null,
  identity: Identity | null,
) {
  return (
    !!channel &&
    !!status &&
    (channel.creator_device_id === status.device_id ||
      !!identity?.membership?.capabilities.includes('channel.manage'))
  );
}

export type SurfaceAvailability = 'available' | 'development';
export function availability(implemented: boolean): SurfaceAvailability {
  return implemented ? 'available' : 'development';
}

export function selectedChannel(channels: Channel[], route: string): Channel | undefined {
  return channels.find((channel) => channel.channel_id === route);
}
