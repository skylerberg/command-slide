<script lang="ts">
  import type { TokenFace, TokenKind } from '../data/types'

  import attackIvory from '../assets/tokens/attack-ivory.png'
  import attackUmber from '../assets/tokens/attack-umber.png'
  import columnMoveIvory from '../assets/tokens/column-move-ivory.png'
  import columnMoveUmber from '../assets/tokens/column-move-umber.png'
  import rowMoveIvory from '../assets/tokens/row-move-ivory.png'
  import rowMoveUmber from '../assets/tokens/row-move-umber.png'

  /** Indexed by owner, so the tuples are [ivory, umber]. Both tokens share one
      attack face; only the movement faces separate rook from bishop. */
  const ART: Record<TokenKind | 'attack', [string, string]> = {
    attack: [attackIvory, attackUmber],
    row: [rowMoveIvory, rowMoveUmber],
    column: [columnMoveIvory, columnMoveUmber],
  }

  interface Props {
    kind: TokenKind
    face: TokenFace
    /** Which side's colours the art carries. */
    owner?: number
    /** Omit to fill the parent, which is how the board sizes its discs. */
    size?: number
  }

  let { kind, face, owner = 1, size }: Props = $props()
</script>

<img
  class="token-icon"
  src={ART[face === 'attack' ? 'attack' : kind][owner]}
  alt=""
  draggable="false"
  style={size === undefined ? undefined : `width:${size}px;height:${size}px`}
/>

<style>
  .token-icon {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
