<script lang="ts">
  import { blankPicture, type Picture } from '../model';
  export let value: Picture = blankPicture();
  export let text = 'C';
  export let color = '#477da3';
  export let size = 40;
</script>

<span
  class="avatar"
  class:square={value.shape === 'square'}
  style:width="{size}px"
  style:height="{size}px"
  style:background={color}
  aria-hidden="true"
>
  {#if value.data}<img
      src={value.data}
      alt=""
      style:object-position="{value.x}% {value.y}%"
      style:transform="scale({value.zoom})"
      style:transform-origin="{value.x}% {value.y}%"
    />{:else}{text.slice(0, 3)}{/if}
</span>
