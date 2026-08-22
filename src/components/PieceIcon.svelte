<script lang="ts">
  import type { PieceKind } from '../data/types'

  interface Props {
    kind: PieceKind
    size?: number
  }

  let { kind, size = 30 }: Props = $props()
</script>

<svg
  class="piece-icon"
  width={size}
  height={size}
  viewBox="0 0 32 32"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if kind === 'swordsman'}
    <path d="M16 2v9" />
    <path d="M11 6.5h10" />
    <path d="M16 30c-6.4-4.2-8.6-9.6-8.6-17.4L16 9.8l8.6 2.8c0 7.8-2.2 13.2-8.6 17.4Z" />
    <path d="M16 15v9" />
  {:else if kind === 'flail'}
    <circle cx="14" cy="19" r="7.5" />
    <path d="M6.5 19h15M14 11.5v15" />
    <path d="M19.5 13.5 25 8h4" />
  {:else if kind === 'spearman'}
    <path d="M16 11v19" />
    <path d="M16 2c-2.6 3.4-4 6-4 8l4 1.6 4-1.6c0-2-1.4-4.6-4-8Z" fill="currentColor" />
  {:else if kind === 'archer'}
    <path d="M10 3c11 6 11 20 0 26" />
    <path d="M10 3v26" />
    <path d="M4 16h24" />
    <path d="m22 11 5 5-5 5" />
  {:else if kind === 'trebuchet'}
    <path d="M3 28h26" />
    <path d="M10 28 16 15l6 13" />
    <path d="M6 21 27 7" />
    <path d="M27 7v6" />
    <circle cx="27" cy="16" r="2.6" fill="currentColor" />
  {:else if kind === 'batteringRam'}
    <path d="M7 12h18l-3-5H10l-3 5Z" />
    <path d="M9 12v11M23 12v11" />
    <path d="M3 18h20" />
    <path d="M3 15.5v5" />
    <circle cx="9" cy="26" r="3" />
    <circle cx="23" cy="26" r="3" />
  {/if}
</svg>

<style>
  .piece-icon {
    display: block;
  }
</style>
