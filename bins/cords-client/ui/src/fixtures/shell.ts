// Development-only entry: mounts the production App with synthetic IPC data.
// Not imported by index.html and not emitted in the desktop production bundle.
import { mockIPC } from '@tauri-apps/api/mocks';
import { mount } from 'svelte';
import App from '../App.svelte';
import { defaults, type Channel, type Contact, type Status } from '../model';
import '../styles.css';

const params = new URLSearchParams(location.search);
if (params.get('text') === '125') document.documentElement.style.fontSize = '20px';
const empty = params.get('state') === 'empty';
const status: Status = {
  account_id: 'fixture-account',
  device_id: 'fixture-device',
  server_id: empty ? '' : 'fixture-server-identity',
  origin: empty ? '' : 'https://workshop.example',
  ownership_state: empty ? '' : 'CLAIMED',
  ownership_generation: 1,
  admission_state: empty ? '' : 'active',
  burned: false,
  join_policy: ['moderator_approval'],
  cursors: {},
};
const preferences = {
  ...defaults(),
  displayName: 'Alex',
  theme: params.get('theme') === 'light' ? 'light' : 'dark',
};
const contacts: Contact[] = ['Alex', 'Morgan', 'Sam'].map((nickname, index) => ({
  authorization: {
    value: {
      account_id: `fixture-account-${index}`,
      device_id: index ? `fixture-device-${index}` : status.device_id,
      generation: 1,
      created_at: 1,
      expires_at: null,
      capabilities: [],
    },
  },
  user_card: {
    value: {
      version: 1,
      account_id: `fixture-account-${index}`,
      device_id: index ? `fixture-device-${index}` : status.device_id,
      nickname,
      avatar: null,
      issued_at: 1,
    },
  },
}));
const channels: Channel[] = empty
  ? []
  : ['general', 'design-workshop', 'announcements', 'public-square'].map((name, index) => ({
      channel_id: `fixture-channel-${index}`,
      name,
      creator_device_id: status.device_id,
      epoch: 4,
      confidentiality_mode: index === 3 ? 'public' : 'encrypted',
      members: contacts,
    }));
const messages = [
  [
    'fixture-device-1',
    'I have a first draft ready for the workshop. The smaller details are starting to come together.',
  ],
  [
    'fixture-device',
    'The new spacing helps. There’s room to read without losing track of the conversation.',
  ],
  [
    'fixture-device-2',
    'Let’s keep the channel names short and put the longer context in the conversation.',
  ],
  ['fixture-device-1', 'Agreed. I’ll bring a couple of options to compare tomorrow.'],
].map(([sender_device_id, body], index) => ({
  message_id: `fixture-message-${index}`,
  sender_device_id,
  body,
  client_timestamp: 1792096200 + index * 120,
}));

mockIPC((command, payload) => {
  const args = payload as { route?: string; action?: { kind: string; route?: string } };
  if (command === 'list_accounts')
    return {
      accounts: [
        {
          account_id: status.account_id,
          nickname: 'Visual fixture (no network)',
          avatar_data: '',
          avatar_shape: 'circle',
        },
      ],
      legacy_vault: false,
    };
  if (command === 'unlock_account') return status;
  if (command === 'conversation_view')
    return {
      status,
      identity: {
        authorization: contacts[0].authorization.value,
        revoked: false,
        membership: { capabilities: [], status: 'active', issued_at: 1 },
        session_expires_at: null,
      },
      preferences,
      connected: !empty,
      error: null,
      channels,
      contacts,
      messages: args.route ? messages : [],
    };
  if (command === 'session_control') return status.account_id;
  if (command === 'conversation_action') {
    switch (args.action?.kind) {
      case 'servers':
        return empty
          ? []
          : [{ ...status, active: true, archived: false, remote_drop_confirmed: false }];
      case 'channels':
        return channels;
      case 'members':
        return contacts;
      case 'authenticate':
        return status;
      case 'join':
        return null;
      default:
        throw new Error(`Visual fixture does not support mutation: ${args.action?.kind}`);
    }
  }
  throw new Error(`Visual fixture does not support command: ${command}`);
});
mount(App, { target: document.getElementById('app')! });
const label = document.createElement('div');
label.textContent = 'VISUAL FIXTURE · NO NETWORK';
label.style.cssText =
  'position:fixed;right:12px;top:8px;z-index:100;font:10px system-ui;letter-spacing:1px;color:#b0bbca;pointer-events:none';
document.body.append(label);
