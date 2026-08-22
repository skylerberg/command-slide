# Command Slide

A two-player siege game on a 7×7 board. Each side commands ten pieces, two
command tokens, and three castles.

## The board

```
       a    b    c    d    e    f    g
    ┌────┬────┬────┬────┬────┬────┬────┐
  7 │ ▣C │ Sp │ Ar │ ▣C │ Ar │ Sp │ ▣C │   Ivory back rank
    ├────┼────┼────┼────┼────┼────┼────┤   (the trebuchet starts on
  6 │    │ Sw │ Fl │ Rm │ Fl │ Sw │    │    the middle castle)
    ├────┼────┼────┼────┼────┼────┼────┤
  5 │    │    │    │    │    │    │    │
    ├────┼────┼────┼────┼────┼────┼────┤
  4 │ ▲H │    │    │ ▲H │    │    │ ▲H │   the hilltops
    ├────┼────┼────┼────┼────┼────┼────┤
  3 │    │    │    │    │    │    │    │
    ├────┼────┼────┼────┼────┼────┼────┤
  2 │    │ Sw │ Fl │ Rm │ Fl │ Sw │    │
    ├────┼────┼────┼────┼────┼────┼────┤
  1 │ ▣C │ Sp │ Ar │ ▣C │ Ar │ Sp │ ▣C │   Umber back rank
    └────┴────┴────┴────┴────┴────┴────┘
```

`Sp` spearman · `Ar` archer · `Sw` swordsman · `Fl` flail · `Rm` battering ram.
The trebuchet (not shown) stands on each side's middle castle at d7 and d1.

**Castles** are terrain, not pieces. They never move and never attack, a piece
may stand on one, and only a siege engine can destroy one. **Hilltops** are the
three middle-row squares a1-side, centre, and g-side — a4, d4 and g4. Castles
and hilltops share files, which is what puts every castle exactly three squares
from a hilltop.

Ivory moves first.

## Winning

Accomplish either one:

* Destroy all three of your opponent's castles.
* Eliminate both of your opponent's siege engines.

The second condition follows from the first: only siege engines damage castles,
so a side with neither can no longer win, and the game is over rather than
drawn out.

If neither side has won after 300 turns the game is a draw. Nothing in the
rules forces progress; the cap exists so that every line terminates.

## Command tokens

Each side has two command tokens, and each token has two faces.

| Token | Rides | Movement face | Attack face |
|---|---|---|---|
| Rank token | the left or right edge, naming a **rank** | move a piece from that rank **like a rook** | every piece of yours on that rank attacks |
| File token | the top or bottom edge, naming a **file** | move a piece from that file **like a bishop** | every piece of yours on that file attacks |

Ivory's tokens ride the left and top edges, Umber's the right and bottom. Both
start showing their movement face.

## A turn

**Your first turn.** Slide either command token to a new line, activate it, and
flip it to its attack face.

**Every turn after.** Slide whichever token still shows its movement face, then
activate and flip **both** tokens, in whatever order you choose.

A token must slide to a *different* line, and that line must hold at least one
piece of yours. Castles do not count — they are terrain.

Because every turn flips both tokens, exactly one token shows a movement face at
the start of each turn after the first. **The token you slid last turn is the
one that fires this turn, from the line you put it on**: every attack is
announced a full turn before it lands, and the standard combination is to move a
piece into a line you armed on the previous turn.

### Activating a movement face

Choose one of your pieces on the named line and move it. The token decides which
line the piece comes *from*; the move itself may carry it anywhere a rook or
bishop could go, including out of that line. Moves never capture and never jump —
a slide stops before any occupied square, friend or enemy. Castle squares are
terrain and do not block.

If no piece on the line has a legal move, the activation passes.

### Activating an attack face

Every piece of yours on the named line attacks at once. **Attackers do not
move.** Nothing blocks an attack — a spearman strikes over the square in front
of it.

Ordinary pieces destroy enemy *pieces* and never damage a castle. Siege engines
destroy enemy *castles* and never touch a piece. Friendly pieces are never hit.

## Attacks

Relative to the piece, `X` marks what it strikes.

```
   swordsman        flail          spearman         archer
   . . X . .      . X . X .        . . X . .      X . . . X
   . X ■ X .      . . ■ . .        . . . . .      . . . . .
   . . X . .      . X . X .        X . ■ . X      . . ■ . .
                                   . . . . .      . . . . .
                                   . . X . .      X . . . X
```

* **Swordsman** — the four squares orthogonally adjacent.
* **Flail** — the four squares diagonally adjacent.
* **Spearman** — two squares away, orthogonally.
* **Archer** — two squares away, diagonally.
* **Trebuchet** — castles exactly three squares away, straight or diagonal, and
  only while it stands on a hilltop. Off a hilltop it throws nothing.
* **Battering Ram** — a castle one square away, straight only.

From the centre hilltop a trebuchet bears on *every* castle on the board, which
makes d4 the sharpest square in the game and the trebuchet's approach the thing
both sides are really playing around.

## Interpretations

The printed rules leave a few things unstated. Where a reading had to be
chosen, this is the one the engine implements:

* **Castles are terrain.** The rules text says three castles and the printed
  board draws two per side — but the back-rank middle square is shaded like the
  castle corners and holds the trebuchet. Castles therefore sit on the back rank
  at files a, d and g, and the trebuchet starts standing on the middle one.
* **Hilltops are the middle row only.** Only from there can a trebuchet reach an
  enemy castle at range three, so back-rank hilltops would be decorative.
* **A slide must change lines.** "Slide the movement token" is read as requiring
  actual movement. If a side's every piece sits on its token's current line it
  has no legal destination; rather than deadlock, the token holds its line.
* **Moves never capture.** "Pieces do not move when attacking" is read as the
  converse too: movement and attack are separate, and a piece may not be taken
  by being moved onto.
* **The trebuchet's range is exactly three**, not up to three.
* **A volley resolves simultaneously.** Every attacker on the line fires before
  anything is removed. Since attackers are always friendly and casualties always
  enemy, resolution order cannot matter.
