<script lang="ts">
  import type { TokenFace, TokenKind } from '../data/types'

  interface Props {
    kind: TokenKind
    face: TokenFace
    size?: number
  }

  let { kind, face, size = 30 }: Props = $props()
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 32 32"
  fill="none"
  stroke="currentColor"
  stroke-width="2.2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if face === 'attack'}
    <!-- A volley: the shaft, and the shock going out both ways. -->
    <path d="M16 9v14" />
    <path d="M11 11a9 9 0 0 0 0 10M21 11a9 9 0 0 1 0 10" />
    <path d="M6.5 7.5a15 15 0 0 0 0 17M25.5 7.5a15 15 0 0 1 0 17" />
  {:else if kind === 'row'}
    <!-- Rook lines: straight out, four ways. -->
    <rect x="12.5" y="12.5" width="7" height="7" />
    <path d="M16 11V4M16 21v7M11 16H4M21 16h7" />
    <path d="M13 6.5 16 3.5l3 3M13 25.5l3 3 3-3M6.5 13l-3 3 3 3M25.5 13l3 3-3 3" />
  {:else}
    <!-- Bishop lines: straight out, four corners. -->
    <rect x="12.5" y="12.5" width="7" height="7" transform="rotate(45 16 16)" />
    <path d="M12 12 6 6M20 12l6-6M12 20l-6 6M20 20l6 6" />
    <path d="M4 9.5 5.5 4.5 10.5 6M27.5 9.5 26.5 4.5 21.5 6M4 22.5 5.5 27.5l5-1.5M27.5 22.5 26.5 27.5l-5-1.5" />
  {/if}
</svg>
