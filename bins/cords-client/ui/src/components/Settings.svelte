<script lang="ts">
  import {
    ExternalLink,
    Fingerprint,
    HardDrive,
    Info,
    KeyRound,
    LockKeyhole,
    Monitor,
    ShieldCheck,
  } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import Avatar from './Avatar.svelte';
  import ImageEditor from './ImageEditor.svelte';
  import type { Preferences, Identity, ServerListing, Status } from '../model';

  export let value: Preferences;
  export let identity: Identity | null;
  export let status: Status | null;
  export let servers: ServerListing[];
  export let close: () => void;
  export let save: (value: Preferences) => Promise<void>;
  export let revoke: () => void;
  export let lockNow: () => Promise<void>;
  export let removeServer: (serverId: string) => void;
  export let archiveServer: (serverId: string) => void;
  export let burnIdentity: () => void;
  export let designateSuccessor: (accountId: string) => Promise<string>;
  export let acceptSuccessor: (designationHash: string) => Promise<void>;

  type Page =
    | 'My Account'
    | 'Identity & devices'
    | 'Profile image'
    | 'Recovery'
    | 'Jewel'
    | 'Server trust'
    | 'Local history'
    | 'Appearance'
    | 'Accessibility'
    | 'Notifications'
    | 'Privacy'
    | 'Blocked Users'
    | 'Connections'
    | 'Devices'
    | 'Sessions'
    | 'Security'
    | 'Advanced';
  const groups: { title: string; pages: Page[] }[] = [
    { title: 'Account', pages: ['My Account', 'Identity & devices', 'Profile image', 'Recovery'] },
    { title: 'App & connections', pages: ['Jewel', 'Server trust', 'Local history'] },
    {
      title: 'Preferences',
      pages: ['Appearance', 'Accessibility', 'Notifications', 'Privacy', 'Blocked Users'],
    },
    {
      title: 'Connections & access',
      pages: ['Connections', 'Devices', 'Sessions', 'Security', 'Advanced'],
    },
  ];
  let draft = structuredClone(value);
  let page: Page = 'Identity & devices';
  let filter = '';
  let error = '';
  let saved = false;
  let busy = false;
  let burnPhrase = '';
  let successorAccount = '';
  let designationHash = '';
  let successorNotice = '';

  async function designationAction(accept: boolean) {
    busy = true;
    error = '';
    try {
      if (accept) {
        await acceptSuccessor(designationHash);
        successorNotice =
          'Successor acceptance recorded. The 30-day period still runs from server acceptance of the designation.';
      } else {
        const hash = await designateSuccessor(successorAccount);
        successorNotice = `Designation accepted. Give this designation hash to the successor: ${hash}`;
      }
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }

  async function persist() {
    busy = true;
    error = '';
    saved = false;
    try {
      await save(draft);
      saved = true;
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  const pending = 'Not implemented yet';
</script>

<Modal title="Cords settings" {close} wide>
  <div class="settings-layout donor-settings">
    <aside class="settings-nav donor-settings-nav">
      <div class="profile-heading">
        <Avatar value={draft.avatar} text={draft.displayName.slice(0, 2)} size={42} /><span
          ><strong>{draft.displayName || 'You'}</strong><small>Local installation</small></span
        >
      </div>
      <label class="settings-search"
        ><span class="sr-only">Search settings</span><input
          type="search"
          placeholder="Search settings"
          bind:value={filter}
        /></label
      >
      {#each groups as group (group.title)}
        <section class="settings-menu-group">
          <h3>{group.title}</h3>
          {#each group.pages.filter((item) => item
              .toLowerCase()
              .includes(filter.toLowerCase())) as item (item)}<button
              class:active={page === item}
              aria-current={page === item ? 'page' : undefined}
              on:click={() => {
                page = item;
                saved = false;
              }}>{item}</button
            >{/each}
        </section>
      {/each}
      <div class="settings-sidebar-note">
        <LockKeyhole size={14} /> Secrets stay in the native client
      </div>
    </aside>
    <section class="settings-body donor-settings-body">
      <div class="settings-topbar"><strong>{page}</strong></div>
      {#if error}<p role="alert" class="error">{error}</p>{/if}

      {#if page === 'My Account'}
        <header class="settings-page-header">
          <h1>My account</h1>
          <p>Public account presentation and local profile controls.</p>
        </header>
        <div class="account-profile-card">
          <div class="account-banner-art"></div>
          <div class="account-profile-main">
            <Avatar value={draft.avatar} text={draft.displayName.slice(0, 2)} size={76} />
            <div>
              <strong>{draft.displayName || 'You'}</strong><span>Portable Cords identity</span
              ><small
                >Nickname, profile image URL and crop are shared when you request membership.</small
              >
            </div>
          </div>
        </div>
        <section>
          <h2>Local profile</h2>
          <label>Display name<input maxlength="100" bind:value={draft.displayName} /></label
          ><ImageEditor title="Profile picture" bind:value={draft.avatar} />
        </section>
      {:else if page === 'Profile image'}
        <header class="settings-page-header">
          <h1>Profile image</h1>
          <p>Choose, validate and crop the image shown by this installation.</p>
        </header>
        <div class="profile-preview-row">
          <Avatar value={draft.avatar} text={draft.displayName.slice(0, 2)} size={76} />
          <div>
            <strong>Local presentation</strong>
            <p>
              The HTTPS URL and crop are public presentation metadata. The normalized preview stays
              encrypted on this installation.
            </p>
          </div>
        </div>
        <ImageEditor title="Profile image source and crop" bind:value={draft.avatar} />
        <div class="account-banner">
          <Info size={19} /><span
            >Signed user cards publish the HTTPS URL and crop, not the locally cached image bytes.</span
          >
        </div>
      {:else if page === 'Jewel'}
        <header class="settings-page-header">
          <h1>The Jewel</h1>
          <p>The small personal mark at the top of the server rail.</p>
        </header>
        <div class="profile-preview-row">
          <Avatar value={draft.jewel} text={draft.jewelText} color={draft.jewelColor} size={76} />
          <div>
            <strong>Preview</strong>
            <p>The Jewel is separate from your account profile image.</p>
          </div>
        </div>
        <div class="admin-form-grid">
          <label>Letter<input maxlength="3" bind:value={draft.jewelText} /></label><label
            >Background<input type="color" bind:value={draft.jewelColor} /></label
          >
        </div>
        <label
          >Click behavior<select bind:value={draft.jewelAction}
            ><option value="dms">Open direct messages</option><option value="settings"
              >Open settings</option
            ><option value="none">Decorative</option></select
          ></label
        >
        <ImageEditor title="Jewel image" bind:value={draft.jewel} />
      {:else if page === 'Identity & devices'}
        <header class="settings-page-header">
          <h1>Portable identity</h1>
          <p>Your account root authorizes devices across independently operated servers.</p>
        </header>
        <div class="identity-fingerprint">
          <span>Account fingerprint</span><code>{status?.account_id ?? 'Unlock to inspect'}</code
          ><small>Derived from the native account-root public identity.</small>
        </div>
        <section>
          <h2>Account authority</h2>
          <div class="identity-authority">
            <KeyRound size={22} />
            <div>
              <strong>Protected by the native client</strong><small
                >The WebView never receives account-root private material.</small
              >
            </div>
            <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
              >Unlock account authority</button
            >
          </div>
        </section>
        <section>
          <h2>Authorized devices</h2>
          <div class="identity-device">
            <Monitor size={22} />
            <div>
              <strong>This installation</strong><code>{status?.device_id}</code><small
                >Generation {identity?.authorization.generation ?? '—'} · {identity?.revoked
                  ? 'Revoked'
                  : 'Root authorized'}</small
              >
            </div>
            {#if !identity?.revoked}<button
                class="danger"
                disabled={!status?.server_id}
                on:click={revoke}>Revoke…</button
              >{/if}
          </div>
          <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
            >Enroll another device</button
          >
        </section>
        <div class="account-banner">
          <Info size={19} /><span
            >Authorizing a device does not grant old MLS conversation keys or server membership.</span
          >
        </div>
      {:else if page === 'Recovery'}
        <header class="settings-page-header">
          <h1>Recovery</h1>
          <p>Recovery must restore account authority from material you control.</p>
        </header>
        <div class="recovery-choice">
          <KeyRound size={22} />
          <div>
            <strong>Account-held recovery</strong><small
              >An encrypted bundle or an existing authorized device requires a separately reviewed
              protocol.</small
            >
          </div>
        </div>
        <div class="settings-action-row">
          <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
            >Import recovery bundle</button
          ><button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
            >Create recovery bundle</button
          >
        </div>
        <div class="account-banner">
          <Info size={19} /><span
            >Server sign-in cannot restore the account root. Account recovery also does not
            automatically restore historical MLS secrets.</span
          >
        </div>
      {:else if page === 'Server trust'}
        <header class="settings-page-header">
          <h1>Server trust</h1>
          <p>
            Web PKI validates the origin; the persistent Cords signing identity is pinned
            separately.
          </p>
        </header>
        <div class="trust-card">
          <div>
            <strong>{status?.origin || 'No trusted server'}</strong><small
              >Current installation</small
            >
          </div>
          <code>{status?.server_id || 'No pin'}</code>
        </div>
        <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
          >Review trust reset</button
        >
        <section class="settings-card">
          <h2>Tracked servers</h2>
          <p>
            Remove erases local messages and MLS state after a signed drop. Archive preserves only
            the origin, fingerprint, and departure status.
          </p>
          {#each servers as server (server.server_id)}
            <div class="trust-card">
              <div>
                <strong>{server.origin}</strong><small
                  >{server.archived
                    ? 'Archived — never auto-reconnects'
                    : server.active
                      ? 'Selected server'
                      : 'Tracked server'}</small
                >
              </div>
              <code>{server.server_id}</code>
            </div>
            {#if server.archived}
              <p>
                {server.remote_drop_confirmed
                  ? 'Remote drop confirmed.'
                  : 'Remote membership removal was not verified.'}
              </p>
            {:else}
              <div class="settings-action-row">
                <button
                  class="danger"
                  disabled={busy}
                  on:click={() => removeServer(server.server_id)}>Remove server…</button
                >
                <button disabled={busy} on:click={() => archiveServer(server.server_id)}
                  >Archive server…</button
                >
              </div>
            {/if}
          {:else}<p>No tracked servers.</p>{/each}
        </section>
        {#if status?.server_id && !status.burned}
          <section class="settings-card">
            <h2>Owner succession</h2>
            <p>
              A successor must already be a server member, explicitly accept with their account
              root, and mature for 30 full days. An accepted designation cannot be removed by the
              owner key alone.
            </p>
            <form on:submit|preventDefault={() => void designationAction(false)}>
              <label
                >Successor account fingerprint<input
                  bind:value={successorAccount}
                  required
                /></label
              >
              <button disabled={busy || !successorAccount}>Designate successor</button>
            </form>
            <form on:submit|preventDefault={() => void designationAction(true)}>
              <label
                >Designation hash to accept<input bind:value={designationHash} required /></label
              >
              <button disabled={busy || !designationHash}>Accept designation</button>
            </form>
            {#if successorNotice}<p role="status">{successorNotice}</p>{/if}
          </section>
        {/if}
      {:else if page === 'Local history'}
        <header class="settings-page-header">
          <h1>Local history</h1>
          <p>
            Decrypted history is cached only on authorized clients and independently protected at
            rest.
          </p>
        </header>
        <div class="settings-other">
          <HardDrive size={30} />
          <h2>Encrypted local cache</h2>
          <p>Old MLS epoch keys are not retained by the displayed-message cache.</p>
        </div>
        <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
          >Configure retention</button
        ><button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
          >Export local history</button
        >
      {:else if page === 'Appearance'}
        <header class="settings-page-header">
          <h1>Appearance</h1>
          <p>Tune the local interface for your screen and reading preference.</p>
        </header>
        <div class="theme-grid">
          <button class:selected={draft.theme === 'dark'} on:click={() => (draft.theme = 'dark')}
            ><i class="theme-dark"></i>Dark</button
          ><button
            class:selected={draft.theme === 'midnight'}
            on:click={() => (draft.theme = 'midnight')}
            ><i class="theme-midnight"></i>Midnight</button
          ><button class:selected={draft.theme === 'light'} on:click={() => (draft.theme = 'light')}
            ><i class="theme-light"></i>Light</button
          >
        </div>
        <label class="toggle-row"
          ><span>Compact message mode<small>Reduce message spacing and avatar size.</small></span
          ><input type="checkbox" bind:checked={draft.compact} /></label
        >
        <label
          >Interface density<select
            class="dev-unimplemented"
            disabled
            aria-disabled="true"
            title={pending}><option>Comfortable</option></select
          ></label
        ><label
          >Font size<select class="dev-unimplemented" disabled aria-disabled="true" title={pending}
            ><option>14 px</option></select
          ></label
        >
      {:else if page === 'Accessibility'}
        <header class="settings-page-header">
          <h1>Accessibility</h1>
          <p>Controls that make conversation and security information easier to use.</p>
        </header>
        <label class="toggle-row"
          ><span>Reduce motion<small>Avoid animated transitions and movement.</small></span><input
            type="checkbox"
            bind:checked={draft.reducedMotion}
          /></label
        >
        <button
          class="dev-unimplemented settings-row-button"
          disabled
          aria-disabled="true"
          title={pending}>High contrast controls</button
        ><button
          class="dev-unimplemented settings-row-button"
          disabled
          aria-disabled="true"
          title={pending}>Always underline links</button
        ><button
          class="dev-unimplemented settings-row-button"
          disabled
          aria-disabled="true"
          title={pending}>High visibility focus indicator</button
        >
      {:else if page === 'Notifications'}
        <header class="settings-page-header">
          <h1>Notifications</h1>
          <p>Choose which local events should get your attention.</p>
        </header>
        {#each ['Desktop notifications', 'Direct messages', 'Mentions', 'Server activity', 'Notification sounds'] as item (item)}<button
            class="dev-unimplemented settings-row-button"
            disabled
            aria-disabled="true"
            title={pending}>{item}</button
          >{/each}
      {:else if page === 'Privacy'}
        <header class="settings-page-header">
          <h1>Privacy</h1>
          <p>
            Messages are encrypted, but servers still observe routing, membership, timing and
            ciphertext size.
          </p>
        </header>
        <div class="account-banner">
          <LockKeyhole size={19} /><span
            >Loading an HTTPS profile image contacts that host directly and may reveal your IP
            address.</span
          >
        </div>
        <label class="toggle-row"
          ><span
            >Genericize Mode<small
              >Use local initials instead of remotely sourced avatars on locked surfaces.</small
            ></span
          ><input type="checkbox" bind:checked={draft.genericize} /></label
        >
        <label class="toggle-row"
          ><span
            >Hide nickname while locked<small
              >Show a numbered local account label on the account picker.</small
            ></span
          ><input type="checkbox" bind:checked={draft.hideNicknameOnLock} /></label
        >
        {#each ['Who can direct message you', 'Contact requests', 'Sensitive media', 'Activity status', 'Link previews'] as item (item)}<button
            class="dev-unimplemented settings-row-button"
            disabled
            aria-disabled="true"
            title={pending}>{item}</button
          >{/each}
      {:else if page === 'Blocked Users'}
        <header class="settings-page-header">
          <h1>Blocked users</h1>
          <p>Locally hide contact and message requests from selected accounts.</p>
        </header>
        <div class="empty">
          <strong>No authoritative block list</strong>
          <p>The current core does not expose blocking state.</p>
        </div>
        <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
          >Block an account</button
        >
      {:else if page === 'Connections'}
        <header class="settings-page-header">
          <h1>Connections</h1>
          <p>Optional public attestations and connected tools.</p>
        </header>
        <div class="connection-card">
          <KeyRound size={23} />
          <div>
            <strong>OpenPGP attestation</strong><span>No attestation connected</span><small
              >Optional external evidence; never a native Cords or conversation key.</small
            >
          </div>
          <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
            >Connect</button
          >
        </div>
        <div class="connection-card">
          <ExternalLink size={23} />
          <div>
            <strong>Git forge</strong><span>Not connected</span><small
              >Future optional profile evidence.</small
            >
          </div>
          <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
            >Connect</button
          >
        </div>
      {:else if page === 'Devices'}
        <header class="settings-page-header">
          <h1>Devices</h1>
          <p>Each authorized installation has its own identity and conversation membership.</p>
        </header>
        <div class="session-list">
          <div>
            <Monitor size={23} /><span
              ><strong>This installation</strong><small>{status?.device_id}</small></span
            ><b>{identity?.revoked ? 'Revoked' : 'Current'}</b>
          </div>
        </div>
        <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
          >Review other devices</button
        >
      {:else if page === 'Sessions'}
        <header class="settings-page-header">
          <h1>Sessions</h1>
          <p>Server sessions authenticate this device to individual communities.</p>
        </header>
        <div class="session-list">
          <div>
            <HardDrive size={23} /><span
              ><strong>{status?.origin || 'No server selected'}</strong><small
                >{identity?.session_expires_at
                  ? `Expires ${new Date(identity.session_expires_at * 1000).toLocaleString()}`
                  : 'No active session'}</small
              ></span
            ><b>{identity?.membership?.status ?? 'Offline'}</b>
          </div>
        </div>
        <button class="dev-unimplemented" disabled aria-disabled="true" title={pending}
          >Sign out this server session</button
        >
      {:else if page === 'Security'}
        <header class="settings-page-header">
          <h1>Security</h1>
          <p>Local protection and account verification state.</p>
        </header>
        <div class="security-score">
          <ShieldCheck size={28} />
          <div>
            <strong>{identity?.revoked ? 'Device revoked' : 'Native protection active'}</strong
            ><span>Encrypted local state and pinned server identity</span>
          </div>
        </div>
        <section>
          <h2>Auto-lock</h2>
          <label
            >Lock after inactivity<select bind:value={draft.autoLockMinutes}
              ><option value={1}>1 minute</option><option value={5}>5 minutes</option><option
                value={15}>15 minutes</option
              ><option value={30}>30 minutes</option><option value={60}>1 hour</option><option
                value={null}>Never</option
              ></select
            ></label
          >
          {#if draft.autoLockMinutes === null}<div class="account-banner warning">
              <Info size={19} /><span
                >Never locking leaves private keys available until you lock or exit Cords.</span
              >
            </div>{/if}
          <label class="toggle-row"
            ><span
              >Lock on operating-system session lock<small
                >Enabled on platforms that report a reliable session-lock event.</small
              ></span
            ><input type="checkbox" bind:checked={draft.lockOnOsLock} /></label
          >
          <label class="toggle-row"
            ><span
              >Lock on system suspend<small
                >A detected suspend gap drops keys and identity-bound connections.</small
              ></span
            ><input type="checkbox" bind:checked={draft.lockOnSuspend} /></label
          >
          <div class="settings-row-button security-invariant">
            Network handling while locked: disconnect all identity-bound connections
          </div>
          <button class="danger" on:click={lockNow}>Lock Now</button>
        </section>
        <div class="settings-row-button security-invariant">
          Local password unlock is required at every startup
        </div>
        <div class="settings-row-button security-invariant">
          Server identity changes fail closed
        </div>
        <section class="settings-card">
          <h2>Burn this identity</h2>
          <p>
            This is for compromise or account deletion only. Cords signs a burn notice for every
            known server and retries failed deliveries. Unknown historical servers cannot be
            notified. The current account cannot resume ordinary messaging after burn begins.
            Servers must manually review any reinvite with extreme caution.
          </p>
          <label
            >Type BURN to continue<input
              bind:value={burnPhrase}
              autocomplete="off"
              spellcheck="false"
            /></label
          >
          <button
            class="danger"
            disabled={status?.burned || burnPhrase !== 'BURN'}
            on:click={burnIdentity}>Burn identity…</button
          >
          {#if status?.burned}
            <p role="status">
              This identity has been burned on this device. Keep this account installed and unlocked
              to deliver pending server notices.
            </p>
            {#each identity?.burn_deliveries ?? [] as delivery (delivery.origin)}
              <p class="hint">
                {delivery.origin}: {delivery.last_error ?? 'Awaiting delivery'} · Next attempt: {delivery.next_retry_at
                  ? new Date(delivery.next_retry_at * 1000).toLocaleString()
                  : 'soon'}
              </p>
            {:else}<p role="status">No known server notices remain pending.</p>{/each}
          {/if}
        </section>
        <button
          class="dev-unimplemented settings-row-button"
          disabled
          aria-disabled="true"
          title={pending}>Verification reminders</button
        >
      {:else}
        <header class="settings-page-header">
          <h1>Advanced</h1>
          <p>Developer-facing diagnostics and local presentation options.</p>
        </header>
        <div class="card">
          <Fingerprint size={22} />
          <p>
            Native Rust owns identities, cryptographic state and sessions. The WebView receives
            public view models and displayed messages only.
          </p>
        </div>
        <button
          class="dev-unimplemented advanced-export"
          disabled
          aria-disabled="true"
          title={pending}><ExternalLink size={16} />Export scrubbed diagnostics</button
        >
      {/if}

      {#if ['My Account', 'Profile image', 'Jewel', 'Appearance', 'Accessibility', 'Privacy', 'Security'].includes(page)}<footer
          class="settings-actions"
        >
          <button class="primary" disabled={busy || !status} on:click={persist}
            >{busy ? 'Saving…' : 'Save local preferences'}</button
          >{#if saved}<span role="status">Saved on this installation</span>{/if}
        </footer>{/if}
    </section>
  </div>
</Modal>
