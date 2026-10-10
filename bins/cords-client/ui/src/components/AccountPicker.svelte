<script lang="ts">
  import { tick } from 'svelte';
  import { KeyRound, Plus, ShieldAlert } from '@lucide/svelte';

  type LocalAccount = {
    account_id: string;
    nickname: string;
    avatar_data: string;
  };
  type Assessment = { weak: boolean; reasons: string[] };

  export let accounts: LocalAccount[] = [];
  export let selected = '';
  export let legacyVault = false;
  export let unlock: (accountId: string, password: string) => Promise<void>;
  export let create: (nickname: string, password: string, allowWeak: boolean) => Promise<void>;
  export let migrate: (
    nickname: string,
    currentPassword: string,
    password: string,
    allowWeak: boolean,
  ) => Promise<void>;
  export let assess: (password: string, nickname: string) => Promise<Assessment>;

  let mode: 'pick' | 'create' | 'migrate' = 'pick';
  let password = '';
  let confirmation = '';
  let currentPassword = '';
  let nickname = '';
  let busy = false;
  let error = '';
  let warning: Assessment | null = null;
  let passwordInput: HTMLInputElement;

  async function choose(accountId: string) {
    selected = accountId;
    mode = 'pick';
    password = '';
    error = '';
    await tick();
    passwordInput?.focus();
  }
  function initials(name: string) {
    return (
      name
        .trim()
        .split(/\s+/u)
        .slice(0, 2)
        .map((part) => part[0] ?? '')
        .join('')
        .toLocaleUpperCase() || 'C'
    );
  }
  async function submitUnlock() {
    if (!selected || busy) return;
    busy = true;
    error = '';
    try {
      await unlock(selected, password);
      password = '';
    } catch (caught) {
      error = String(caught);
      password = '';
      await tick();
      passwordInput?.focus();
    } finally {
      busy = false;
    }
  }
  async function submitNew(allowWeak = false) {
    error = '';
    if (password !== confirmation) {
      error = 'Passwords do not match.';
      return;
    }
    const result = await assess(password, nickname);
    if (result.weak && !allowWeak) {
      warning = result;
      return;
    }
    busy = true;
    try {
      if (mode === 'migrate') await migrate(nickname, currentPassword, password, allowWeak);
      else await create(nickname, password, allowWeak);
      password = '';
      confirmation = '';
      currentPassword = '';
      warning = null;
    } catch (caught) {
      error = String(caught);
      password = '';
      confirmation = '';
    } finally {
      busy = false;
    }
  }
  function resetForm(next: 'pick' | 'create' | 'migrate') {
    mode = next;
    selected = '';
    password = '';
    confirmation = '';
    currentPassword = '';
    nickname = '';
    error = '';
    warning = null;
  }
  function escape(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    if (mode !== 'pick') resetForm('pick');
    else {
      selected = '';
      password = '';
      error = '';
    }
  }
</script>

<svelte:window on:keydown={escape} />

<main class="account-picker-shell">
  <section class="account-picker" aria-labelledby="account-picker-title">
    <p class="eyebrow">Portable identity</p>
    <h1 id="account-picker-title">Who’s using Cords?</h1>
    <p class="picker-intro">
      Choose a local identity. Private account details remain encrypted until unlock.
    </p>

    {#if mode === 'pick'}
      <div class="account-tiles" role="list" aria-label="Local Cords accounts">
        {#each accounts as account (account.account_id)}
          <div class="account-choice" role="listitem">
            <button
              class="account-tile"
              class:selected={selected === account.account_id}
              aria-pressed={selected === account.account_id}
              aria-label={`Unlock ${account.nickname}`}
              on:click={() => choose(account.account_id)}
            >
              <span class="picker-avatar">
                {#if account.avatar_data}<img src={account.avatar_data} alt="" />{:else}<b
                    >{initials(account.nickname)}</b
                  >{/if}
              </span>
              <strong>{account.nickname}</strong>
            </button>
            {#if selected === account.account_id}
              <form class="tile-unlock" on:submit|preventDefault={submitUnlock}>
                <label>
                  <span class="sr-only">Local storage password for {account.nickname}</span>
                  <input
                    bind:this={passwordInput}
                    bind:value={password}
                    type="password"
                    autocomplete="current-password"
                    placeholder="Password"
                    required
                  />
                </label>
                <button class="primary" disabled={busy || !password}>
                  <KeyRound size={17} />{busy ? 'Unlocking…' : 'Unlock'}
                </button>
              </form>
            {/if}
          </div>
        {/each}
        <button class="account-tile add-account" on:click={() => resetForm('create')}>
          <span class="picker-avatar"><Plus size={34} /></span><strong>Add Account</strong>
        </button>
      </div>
      {#if legacyVault}<button class="legacy-migration" on:click={() => resetForm('migrate')}
          >Migrate existing installation</button
        >{/if}
    {:else}
      <form class="account-form" on:submit|preventDefault={() => submitNew(false)}>
        <header>
          <button type="button" on:click={() => resetForm('pick')}>Back</button>
          <div>
            <h2>{mode === 'migrate' ? 'Migrate existing identity' : 'Create local account'}</h2>
            <p>
              {mode === 'migrate'
                ? 'The original vault stays in place until the copied identity has been verified.'
                : 'This password protects this account’s local keys and history.'}
            </p>
          </div>
        </header>
        <label
          >Nickname<input
            bind:value={nickname}
            maxlength="100"
            required
            autocomplete="nickname"
          /></label
        >
        {#if mode === 'migrate'}<label
            >Current installation passphrase<input
              bind:value={currentPassword}
              type="password"
              autocomplete="current-password"
            /></label
          >{/if}
        <label
          >New local storage password<input
            bind:value={password}
            type="password"
            minlength="12"
            autocomplete="new-password"
            required
          /></label
        >
        <label
          >Confirm password<input
            bind:value={confirmation}
            type="password"
            minlength="12"
            autocomplete="new-password"
            required
          /></label
        >
        <p class="hint">
          Use at least 12 characters. Spaces, Unicode, and long passphrases are supported.
        </p>
        <button class="primary" disabled={busy}
          >{busy
            ? 'Protecting account…'
            : mode === 'migrate'
              ? 'Copy and verify identity'
              : 'Create Account'}</button
        >
      </form>
    {/if}

    {#if warning}
      <section class="password-warning" role="alert">
        <ShieldAlert size={24} />
        <div>
          <strong>This password appears weak</strong>
          <p>
            Weak passwords make encrypted local storage easier to attack if its ciphertext is
            stolen.
          </p>
          <ul>
            {#each warning.reasons as reason (reason)}<li>{reason}</li>{/each}
          </ul>
          <a
            href="https://www.hivesystems.com/blog/are-your-passwords-in-the-green"
            target="_blank"
            rel="noreferrer">Learn how password length affects cracking resistance</a
          >
          <div class="actions">
            <button on:click={() => (warning = null)}>Choose another</button>
            <button class="danger" on:click={() => submitNew(true)}
              >I understand, use it anyway</button
            >
          </div>
        </div>
      </section>
    {/if}
    {#if error}<div class="error" role="alert">
        <strong>Account could not be opened</strong>
        <p>{error}</p>
      </div>{/if}
  </section>
</main>
