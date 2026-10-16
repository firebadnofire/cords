<script lang="ts">
  export let error: string;
  export let task: string;
  $: denied = /CORDS_PERMISSION_DENIED|CORDS_FORBIDDEN|HTTP 403/.test(error);
</script>

<section class="request-error" role="alert">
  <strong>{denied ? 'Access restricted' : `Could not ${task}`}</strong>
  <p>
    {denied
      ? 'This server has not granted your account permission for this action. Role administration and its history are currently limited to the server owner while role enforcement is being completed.'
      : 'The request did not complete. Check the connection and try again.'}
  </p>
  <details><summary>Technical details</summary><code>{error}</code></details>
</section>
