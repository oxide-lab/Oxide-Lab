<script lang="ts">
  import { Microphone } from 'phosphor-svelte';
  type Props = {
    class?: string;
    size?: number;
  };

  let { class: className = '', size = 16, ...restProps }: Props = $props();
</script>

<Microphone class={className} {size} {...restProps} />
