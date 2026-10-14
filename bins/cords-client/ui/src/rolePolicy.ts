export type Role = {
  id: string;
  name: string;
  color: string;
  description: string;
  priority: number;
  permissions: string[];
  mentionable: boolean;
};
export type RoleCatalog = {
  version: number;
  server_id: string;
  revision: number;
  active: boolean;
  roles: Role[];
};
export type AccountRoles = {
  account_id: string;
  catalog: RoleCatalog;
  role_ids: string[];
};
export type RoleChange =
  | { operation: 'initialize' }
  | { operation: 'put_role'; role: Role }
  | { operation: 'delete_role'; role_id: string }
  | { operation: 'assign_role' | 'revoke_role'; account_id: string; role_id: string };
export type RoleRequest = {
  expected_revision: number;
  request_id: string;
  change: RoleChange;
};
export type RoleReceipt = { revision: number; request_id: string };
export type RoleAuditEntry = {
  revision: number;
  actor_device_id: string;
  operation: string;
  target: string;
  committed_at: number;
};
export type RoleAuditPage = {
  version: number;
  server_id: string;
  after_revision: number;
  head_revision: number;
  entries: RoleAuditEntry[];
};

export const permissionGroups: { name: string; permissions: [string, string][] }[] = [
  {
    name: 'Server',
    permissions: [
      ['server.read', 'View server'],
      ['server.manage', 'Manage server'],
      ['server.roles.manage', 'Manage roles'],
      ['server.roles.assign', 'Assign roles'],
      ['server.invite.create', 'Create invitations'],
      ['server.invite.manage', 'Manage invitations'],
      ['server.member.approve', 'Approve accounts'],
      ['server.device.approve', 'Approve devices'],
      ['server.audit.read', 'View audit log'],
    ],
  },
  {
    name: 'Channels',
    permissions: [
      ['channel.view', 'View channels'],
      ['channel.read', 'Read messages'],
      ['channel.write', 'Send messages'],
      ['channel.create', 'Create channels'],
      ['channel.manage', 'Edit channel settings'],
      ['channel.retire', 'Retire channels'],
      ['channel.permissions.manage', 'Manage channel permissions'],
      ['channel.members.manage', 'Manage channel members'],
      ['channel.mls.commit', 'Commit MLS changes'],
      ['keypackage.publish', 'Publish device KeyPackages'],
      ['channel.emojis.manage', 'Manage emojis'],
    ],
  },
  {
    name: 'Moderation',
    permissions: [
      ['server.member.kick', 'Remove members'],
      ['server.member.ban', 'Ban members'],
      ['server.member.timeout', 'Timeout members'],
      ['server.moderation.read', 'View moderation'],
      ['channel.messages.manage', 'Manage messages'],
    ],
  },
  {
    name: 'Mentions',
    permissions: [
      ['mention_roles', 'Notify mentionable roles'],
      ['server.roles.mentionability', 'Manage role mentionability'],
    ],
  },
];

export const defaultPermissions = new Set([
  'server.read',
  'channel.view',
  'channel.read',
  'channel.write',
  'channel.mls.commit',
  'keypackage.publish',
  'mention_roles',
]);
