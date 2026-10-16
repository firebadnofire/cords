<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { loadProfile, rememberProfile, type ProfileView, type ProfileAvatar } from '../profiles';
  import ProfileImageEditor from './ProfileImageEditor.svelte';
  import JxlCanvas from './JxlCanvas.svelte';
  export let account: string;
  export let initialName = 'You';
  export let dirty = false;
  let view: ProfileView | undefined;
  let name = initialName;
  let chosen: { avatar: ProfileAvatar | null; image: string | null } | undefined;
  let loading = true,
    busy = false,
    processing = false;
  let error = '',
    notice = '';
  let reviewedName = false,
    reviewedAvatar = false;
  let playback = false;
  $: dirty =
    processing || !!chosen || (!!view && name !== (view.state.agreed?.display_name ?? initialName));
  $: conflicts = view?.state.conflicts ?? [];
  async function refresh() {
    loading = true;
    error = '';
    try {
      view = await loadProfile(account, true);
      name = view.state.agreed?.display_name ?? initialName;
      chosen = undefined;
      reviewedName = reviewedAvatar = false;
    } catch (caught) {
      error = String(caught);
    } finally {
      loading = false;
    }
  }
  async function publish() {
    if (!view || busy || processing) return;
    busy = true;
    error = '';
    notice = '';
    try {
      const result = await invoke<ProfileView>('conversation_action', {
        action: {
          kind: 'save_account_profile',
          profile: {
            display_name: name.trim(),
            avatar: chosen ? chosen.avatar : (view.state.agreed?.avatar ?? null),
          },
          image: chosen?.image ?? null,
          expected_heads: view.state.heads,
        },
      });
      rememberProfile(account, result);
      view = result;
      chosen = undefined;
      name = result.state.agreed?.display_name ?? name;
      reviewedName = reviewedAvatar = false;
      notice =
        result.pending_publication ??
        'Profile published. Other accounts on this server can now see your picture.';
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  async function selectAvatar(avatar: ProfileAvatar | null) {
    error = '';
    busy = true;
    try {
      const image = avatar
        ? await invoke<string>('conversation_action', {
            action: { kind: 'account_profile_image', account, sha256: avatar.sha256 },
          })
        : null;
      chosen = { avatar, image };
      reviewedAvatar = true;
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = false;
    }
  }
  onMount(() => {
    void refresh();
  });
</script>

<header class="settings-page-header">
  <h1>My account</h1>
  <p>Your device signs one public account profile shared with your joined servers.</p>
</header>
{#if loading}<p role="status">Loading verified profile…</p>
{:else if view}
  {#if view.image && view.state.agreed?.avatar}<div
      class="profile-animation-review"
      class:square={view.state.agreed.avatar.shape === 'square'}
    >
      <JxlCanvas image={view.image} size={160} explicit={playback} />
    </div>
    <button on:click={() => (playback = !playback)}
      >{playback ? 'Use motion preference' : 'Play profile animation'}</button
    >{/if}
  {#if !view.state.heads.length}<p class="info">
      This account has no signed public profile yet. Review your display name and choose a picture
      before publishing. Old device previews are not uploaded automatically.
    </p>{/if}
  {#if conflicts.length}<p class="info">
      Concurrent profile edits need review. The last agreed profile remains visible. Choose the name
      and the complete picture field you want to publish; saving resolves all currently reviewed
      heads.
    </p>{/if}
  {#if conflicts.length}
    <section aria-label="Concurrent profile candidates">
      <h2>Choose fields to resolve</h2>
      {#each view.candidates ?? [] as candidate (candidate.id)}
        <div class="connection-card">
          <div>
            <strong>{candidate.profile.display_name}</strong><small
              >Revision {candidate.id.slice(0, 12)}</small
            >
          </div>
          {#if conflicts.includes('display_name')}<button
              disabled={busy}
              on:click={() => {
                name = candidate.profile.display_name;
                reviewedName = true;
              }}>Use this name</button
            >{/if}
          {#if conflicts.includes('avatar')}<button
              disabled={busy}
              on:click={() => selectAvatar(candidate.profile.avatar)}
              >Review {candidate.profile.avatar
                ? `picture ${candidate.profile.avatar.sha256.slice(0, 8)}`
                : 'initials'}</button
            >{/if}
        </div>
      {/each}
    </section>
  {/if}
  <label>Public display name<input maxlength="100" bind:value={name} disabled={busy} /></label>
  {#if conflicts.includes('display_name')}<label
      ><input type="checkbox" bind:checked={reviewedName} />I reviewed and selected this display
      name</label
    >{/if}
  <ProfileImageEditor bind:selected={chosen} bind:busy={processing} />
  {#if conflicts.includes('avatar')}<label
      ><input type="checkbox" bind:checked={reviewedAvatar} />Use {chosen
        ? 'the reviewed picture above'
        : 'the last agreed picture'} for this resolution</label
    >{/if}
  <div class="actions">
    <button
      class="primary"
      on:click={publish}
      disabled={busy ||
        processing ||
        !name.trim() ||
        view.state.pending.length > 0 ||
        (conflicts.includes('display_name') && !reviewedName) ||
        (conflicts.includes('avatar') && !reviewedAvatar)}
      >{busy
        ? 'Publishing…'
        : conflicts.length
          ? 'Sign and publish resolution'
          : 'Publish profile'}</button
    ><button on:click={refresh} disabled={busy || processing}>Discard edits and refresh</button>
  </div>
  {#if view.pending_publication}<p role="status">{view.pending_publication}</p>{/if}
{/if}
{#if notice}<p role="status">{notice}</p>{/if}
{#if error}<p class="error" role="alert">{error}</p>
  <button on:click={refresh} disabled={busy}>Retry profile synchronization</button>{/if}
