<script lang="ts">
  import type { PieceKind } from '../data/types'

  import archerIvory from '../assets/pieces/archer-ivory.png'
  import archerUmber from '../assets/pieces/archer-umber.png'
  import batteringRamIvory from '../assets/pieces/batteringRam-ivory.png'
  import batteringRamUmber from '../assets/pieces/batteringRam-umber.png'
  import flailIvory from '../assets/pieces/flail-ivory.png'
  import flailUmber from '../assets/pieces/flail-umber.png'
  import spearmanIvory from '../assets/pieces/spearman-ivory.png'
  import spearmanUmber from '../assets/pieces/spearman-umber.png'
  import swordsmanIvory from '../assets/pieces/swordsman-ivory.png'
  import swordsmanUmber from '../assets/pieces/swordsman-umber.png'
  import trebuchetIvory from '../assets/pieces/trebuchet-ivory.png'
  import trebuchetUmber from '../assets/pieces/trebuchet-umber.png'

  /** Indexed by owner, so the tuples are [ivory, umber]. */
  const ART: Record<PieceKind, [string, string]> = {
    swordsman: [swordsmanIvory, swordsmanUmber],
    flail: [flailIvory, flailUmber],
    spearman: [spearmanIvory, spearmanUmber],
    archer: [archerIvory, archerUmber],
    trebuchet: [trebuchetIvory, trebuchetUmber],
    batteringRam: [batteringRamIvory, batteringRamUmber],
  }

  interface Props {
    kind: PieceKind
    /** Which side's colours the art carries. Off the board — the menu crest,
        the rules sheet — nothing owns the piece, and umber reads best on
        parchment. */
    owner?: number
    /** Omit to fill the parent, which is how the board sizes its discs. */
    size?: number
  }

  let { kind, owner = 1, size }: Props = $props()
</script>

<img
  class="piece-icon"
  src={ART[kind][owner]}
  alt=""
  draggable="false"
  style={size === undefined ? undefined : `width:${size}px;height:${size}px`}
/>

<style>
  .piece-icon {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
