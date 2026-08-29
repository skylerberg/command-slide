<script lang="ts">
  interface Props {
    kind: 'castle' | 'hilltop' | 'wall'
    /** Omit to fill the parent, which is how the board scales terrain with
        the cell. */
    size?: number
  }

  let { kind, size }: Props = $props()
</script>

<svg
  class="terrain-icon"
  style={size === undefined ? undefined : `width:${size}px;height:${size}px`}
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
  {:else if kind === 'wall'}
    <!-- A block of stone set in the road. Filled rather than drawn, so it
         still reads as solid against the square's tint. -->
    <circle cx="16" cy="16" r="11" fill="currentColor" stroke="none" />
    <path d="M11.5 12a9 9 0 0 1 4.5-2.7" stroke="#fff" opacity="0.3" />
  {:else}
    <path d="M3 24c4-8 8.5-8 12 0" />
    <path d="M11 27c4.5-8 10-8.5 18 0" />
    <path d="M4 27.5h6" />
  {/if}
</svg>

<style>
  .terrain-icon {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
