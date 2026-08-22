<script lang="ts">
  interface Props {
    kind: 'castle' | 'ruin' | 'hilltop'
    size?: number
  }

  let { kind, size = 34 }: Props = $props()
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 32 32"
  fill="none"
  stroke="currentColor"
  stroke-width="1.7"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if kind === 'castle'}
    <path d="M6 28V13h3v-3h3v3h3v-3h3v3h3v-3h3v3h2v15Z" />
    <path d="M13.5 28v-6a2.5 2.5 0 0 1 5 0v6" />
    <path d="M16 10V4h5l-1.6 1.8L21 7.6h-5" />
  {:else if kind === 'ruin'}
    <path d="M6 28V17l3 3 2-4 3 5 2-7 3 6 2-3 3 4v7Z" opacity="0.75" />
  {:else}
    <path d="M3 24c4-8 8.5-8 12 0" />
    <path d="M11 27c4.5-8 10-8.5 18 0" />
    <path d="M4 27.5h6" />
  {/if}
</svg>
