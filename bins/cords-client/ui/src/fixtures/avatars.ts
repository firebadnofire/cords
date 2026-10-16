// Development-only playback evidence. No account, authentication or server mock.
import { mount } from 'svelte';
import AvatarPlaybackFixture from './AvatarPlaybackFixture.svelte';
import '../styles.css';
mount(AvatarPlaybackFixture, { target: document.getElementById('app')! });
