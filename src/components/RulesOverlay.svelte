<script lang="ts">
  import PieceIcon from './PieceIcon.svelte'
  import TokenIcon from './TokenIcon.svelte'
  import { PIECE_NAMES, attackOffsets } from '../data/types'
  import type { PieceKind } from '../data/types'

  interface Props {
    onclose: () => void
  }

  let { onclose }: Props = $props()

  const FIGHTERS: PieceKind[] = ['swordsman', 'flail', 'spearman', 'archer']
  const RANGE = [-3, -2, -1, 0, 1, 2, 3]

  function strikes(kind: PieceKind, drow: number, dcol: number): boolean {
    return attackOffsets(kind).some(([r, c]) => r === drow && c === dcol)
  }
</script>

<div
  class="scrim"
  role="button"
  tabindex="-1"
  onclick={onclose}
  onkeydown={(event) => event.key === 'Escape' && onclose()}
>
  <div
    class="panel sheet"
    role="dialog"
    aria-label="Rules"
    tabindex="-1"
    onclick={(event) => event.stopPropagation()}
    onkeydown={() => {}}
  >
    <header>
      <h2>Command Slide</h2>
      <button onclick={onclose}>Close</button>
    </header>

    <section>
      <h3>Winning</h3>
      <p>Do either one:</p>
      <ul>
        <li>Destroy all three of your opponent's castles.</li>
        <li>Eliminate both of your opponent's siege engines.</li>
      </ul>
      <p class="aside">
        Only siege engines damage castles, and only ordinary pieces damage pieces. Take both of a
        side's siege engines and they can never raze a castle again — which is why that ends it.
      </p>
    </section>

    <section>
      <h3>The ground</h3>
      <ul>
        <li>
          You may stand a piece on your own castle — the trebuchet opens the game doing exactly
          that — but never on an opponent's. An enemy castle stops a slide dead: a piece may
          neither enter it nor pass through it.
        </li>
        <li>
          A destroyed castle takes a hilltop token, and the square becomes high ground either side
          may occupy. Raze a corner castle and a trebuchet standing in its rubble bears on the
          middle one, three squares along the back rank.
        </li>
      </ul>
    </section>

    <section>
      <h3>A turn</h3>
      <p>
        You own two command tokens. One rides the rank track and one the file track, and each has a
        movement face and an attack face.
      </p>
      <ul>
        <li>
          <strong>Your first turn:</strong> slide either token to a new line, activate it, and flip
          it to its attack face.
        </li>
        <li>
          <strong>Every turn after:</strong> slide whichever token still shows its movement face,
          then activate and flip <em>both</em> tokens in whatever order you choose.
        </li>
      </ul>
      <p>A token may only slide to a line that holds a piece of yours, and it must change lines.</p>
      <p class="aside">
        So the token you slid last turn is the one that fires this turn, from the line you put it
        on. Every attack is announced a full turn before it lands.
      </p>
    </section>

    <section>
      <h3>The faces</h3>
      <div class="faces">
        <div>
          <span><TokenIcon kind="row" face="movement" size={26} /></span>
          <span>Move one piece from that <strong>rank</strong>, like a rook.</span>
        </div>
        <div>
          <span><TokenIcon kind="column" face="movement" size={26} /></span>
          <span>Move one piece from that <strong>file</strong>, like a bishop.</span>
        </div>
        <div>
          <span><TokenIcon kind="row" face="attack" size={26} /></span>
          <span>
            Every piece of yours on that line may attack, <strong>one target each</strong>.
            Attackers do not move.
          </span>
        </div>
      </div>
      <p class="aside">
        The token picks which line a piece comes <em>from</em>; the move itself may carry it
        anywhere a rook or bishop could go. Moves never capture, and never jump. You may also flip
        a token and take no action — with a volley, one attacker at a time.
      </p>
    </section>

    <section>
      <h3>Attacks</h3>
      <p>
        A volley works along its line, and each piece with something in range picks a single square
        to hit or holds its fire. Nothing blocks a shot: a spearman strikes over the square in
        front of it.
      </p>
      <div class="patterns">
        {#each FIGHTERS as kind (kind)}
          <figure>
            <div class="mini">
              {#each RANGE.slice(1, 6) as drow (drow)}
                {#each RANGE.slice(1, 6) as dcol (dcol)}
                  <span
                    class="cell"
                    class:hit={strikes(kind, drow, dcol)}
                    class:self={drow === 0 && dcol === 0}
                  >
                    {#if drow === 0 && dcol === 0}
                      <PieceIcon {kind} size={20} />
                    {/if}
                  </span>
                {/each}
              {/each}
            </div>
            <figcaption>{PIECE_NAMES[kind]}</figcaption>
          </figure>
        {/each}
      </div>
      <ul class="siege">
        <li>
          <span class="engine"><PieceIcon kind="trebuchet" size={20} /></span>
          <span>
            <strong>Trebuchet</strong> — hits castles exactly three squares away, straight or
            diagonal, and only while it stands on a hilltop. From the centre hilltop it bears on
            every castle on the board, and brings one of them down a turn.
          </span>
        </li>
        <li>
          <span class="engine"><PieceIcon kind="batteringRam" size={20} /></span>
          <span><strong>Battering Ram</strong> — hits a castle one square away, straight only.</span>
        </li>
      </ul>
    </section>

    <section>
      <h3>Reading the board</h3>
      <ul class="marks">
        <li><i class="key own"></i> a piece or castle your armed token has a shot at</li>
        <li><i class="key own-reach"></i> every square that line covers, occupied or not</li>
        <li><i class="key threat"></i> what the opponent's armed token has a shot at</li>
        <li><i class="key threat-reach"></i> the squares it covers — move into one and it has a
          shot at you</li>
        <li><i class="key movable"></i> a piece this activation may move</li>
        <li><i class="key dest"></i> somewhere it may go</li>
        <li><i class="key kill"></i> a destination that would put a target under your volley</li>
      </ul>
      <p class="aside">
        Each attacker takes one shot, so a line covering four things still only takes as many as
        it has pieces to fire. Hover a slide target or a destination and the line under the board
        says what it leads to; both sets of hints have a switch in the top bar. Decisions that
        cannot change the position — a forced pass, a lone slide, an activation order that leads
        to the same board either way — are taken for you and recorded in the dispatches.
      </p>
    </section>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 20;
    background: rgba(51, 36, 15, 0.5);
    display: grid;
    place-items: center;
    padding: 1rem;
    border: none;
  }

  .sheet {
    max-width: 46rem;
    max-height: 90vh;
    overflow-y: auto;
    padding: 1.2rem 1.4rem 1.6rem;
    display: grid;
    gap: 1.1rem;
    text-align: left;
    cursor: default;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    position: sticky;
    top: -1.2rem;
    background: var(--panel);
    padding: 0.4rem 0;
    margin: -0.4rem 0;
  }

  h2 {
    font-size: 1.5rem;
  }

  .marks {
    display: grid;
    gap: 0.35rem;
    list-style: none;
    padding: 0;
  }

  .marks li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .key {
    width: 14px;
    height: 14px;
    border-radius: 2px;
    flex: none;
  }

  .key.own {
    border: 2px solid var(--gold);
    background: rgba(185, 143, 46, 0.2);
  }

  .key.own-reach {
    border: 1px solid var(--gold-soft);
    background: repeating-linear-gradient(
      -45deg,
      rgba(185, 143, 46, 0.55) 0 2px,
      transparent 2px 6px
    );
  }

  .key.threat {
    border: 2px solid var(--crimson);
    background: rgba(163, 34, 34, 0.22);
  }

  .key.threat-reach {
    border: 1px solid rgba(163, 34, 34, 0.4);
    background: repeating-linear-gradient(
      45deg,
      rgba(163, 34, 34, 0.5) 0 2px,
      transparent 2px 6px
    );
  }

  .key.movable {
    box-shadow: inset 0 0 0 2px var(--gold);
  }

  .key.dest {
    border-radius: 50%;
    background: var(--azure);
    opacity: 0.55;
  }

  .key.kill {
    border-radius: 50%;
    background: var(--crimson);
    opacity: 0.7;
  }

  h3 {
    font-size: 1rem;
    color: var(--ink-soft);
    margin-bottom: 0.3rem;
  }

  ul {
    padding-left: 1.1rem;
    display: grid;
    gap: 0.2rem;
  }

  p + p,
  p + ul,
  ul + p {
    margin-top: 0.4rem;
  }

  .aside {
    color: var(--ink-soft);
    font-style: italic;
    border-left: 2px solid var(--gold-soft);
    padding-left: 0.7rem;
  }

  .faces {
    display: grid;
    gap: 0.5rem;
  }

  .faces > div {
    display: grid;
    grid-template-columns: 2rem 1fr;
    align-items: center;
    gap: 0.6rem;
  }

  .patterns {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
  }

  figure {
    display: grid;
    gap: 0.3rem;
    justify-items: center;
  }

  .mini {
    display: grid;
    grid-template-columns: repeat(5, 24px);
    grid-template-rows: repeat(5, 24px);
  }

  .cell {
    border: 1px solid var(--board-line);
    display: grid;
    place-items: center;
  }

  .cell.hit {
    background: rgba(163, 34, 34, 0.75);
  }

  .cell.self {
    background: var(--castle-tint);
  }

  figcaption {
    font-family: var(--font-display);
    font-size: 0.75rem;
    letter-spacing: 0.06em;
    color: var(--ink-soft);
  }

  .siege {
    list-style: none;
    padding: 0;
    gap: 0.5rem;
    margin-top: 0.7rem;
  }

  .siege li {
    display: grid;
    grid-template-columns: 1.6rem 1fr;
    gap: 0.6rem;
    align-items: start;
  }
</style>
