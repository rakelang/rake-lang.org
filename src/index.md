<div class="home-intro">
<img class="home-intro-image" src="/images/wallpaper.webp" alt="The Rake mark, a rake's head drawn as a grid of tines, above furrows of raked sand" width="1024" height="1536">
<div>
<h1>Rake</h1>
<p>What Rust does for safety with <code>unsafe {}</code>, Rake does for speed with <code>slow {}</code>.</p>
<p>Rust's type system and borrow checker enforce memory safety in safe code. An <code>unsafe</code> block marks operations whose safety the programmer must establish. Rake's compiler checks that vector calculations become vector instructions. If it can't keep a calculation vectorised, compilation fails. A <code>slow { ... }</code> block makes an explicit place for scalar work, and vector code resumes after the closing brace.</p>
<p>Rake is a SIMD, or vector, programming language. It's built for calculations that apply the same operation to many numbers at once.</p>
<ul class="link-row">
<li><a class="link-button" href="/docs/">Documentation</a></li>
<li><a class="link-button" href="/docs/playground/">Tutorial</a></li>
<li><a class="link-button" href="https://github.com/rakelang/rake">Compiler on GitHub</a></li>
</ul>
</div>
</div>

## A first look

Rake's execution model can be pictured in a sentence: “`rake` data `through`
`tine`s, then `sweep` them into the result.” We'll read those words in the
order they appear in a program, starting with what vector processing means.

### One instruction, several numbers

<figure class="diagram">
<div class="diagram-panels">
<div class="diagram-panel">
<h4>Scalar: one number at a time</h4>
<svg viewBox="0 0 320 288" role="img" aria-labelledby="scalar-desk-title scalar-desk-description">
<title id="scalar-desk-title">One scalar add at a narrow desk</title>
<desc id="scalar-desk-description">The tape holds 4, 7, 2 and 9. Only 4 is on the narrow desk. One add instruction changes it to 5, leaving the other numbers unchanged. The tape must advance before the next number can be processed.</desc>
<text class="diagram-label" x="18" y="25">Narrow desk</text>
<rect class="diagram-desk" x="18" y="40" width="66" height="208" rx="8"/>
<rect class="diagram-cell diagram-cell-active" x="24" y="54" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-idle" x="90" y="54" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-idle" x="156" y="54" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-idle" x="222" y="54" width="54" height="44" rx="4"/>
<text class="diagram-value" x="51" y="76">4</text>
<text class="diagram-value diagram-value-idle" x="117" y="76">7</text>
<text class="diagram-value diagram-value-idle" x="183" y="76">2</text>
<text class="diagram-value diagram-value-idle" x="249" y="76">9</text>
<path class="diagram-flow" d="M51 104 V128 M51 164 V182 M46 176 L51 182 L56 176"/>
<rect class="diagram-operation" x="26" y="130" width="50" height="32" rx="4"/>
<text class="diagram-value" x="51" y="146">+1</text>
<rect class="diagram-cell diagram-cell-active" x="24" y="190" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-idle" x="90" y="190" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-idle" x="156" y="190" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-idle" x="222" y="190" width="54" height="44" rx="4"/>
<text class="diagram-value" x="51" y="212">5</text>
<text class="diagram-value diagram-value-idle" x="117" y="212">7</text>
<text class="diagram-value diagram-value-idle" x="183" y="212">2</text>
<text class="diagram-value diagram-value-idle" x="249" y="212">9</text>
<text class="diagram-label" x="18" y="277">Advance the tape, then add again.</text>
</svg>
</div>
<div class="diagram-panel">
<h4>Vector: four numbers together</h4>
<svg viewBox="0 0 320 288" role="img" aria-labelledby="vector-desk-title vector-desk-description">
<title id="vector-desk-title">One vector add at a wide desk</title>
<desc id="vector-desk-description">All four numbers, 4, 7, 2 and 9, fit on the wide desk. One vector add instruction applies plus 1 to all four lanes, producing 5, 8, 3 and 10.</desc>
<text class="diagram-label" x="18" y="25">Wide desk</text>
<rect class="diagram-desk" x="18" y="40" width="288" height="208" rx="8"/>
<rect class="diagram-cell diagram-cell-active" x="24" y="54" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="90" y="54" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="156" y="54" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="222" y="54" width="54" height="44" rx="4"/>
<text class="diagram-value" x="51" y="76">4</text>
<text class="diagram-value" x="117" y="76">7</text>
<text class="diagram-value" x="183" y="76">2</text>
<text class="diagram-value" x="249" y="76">9</text>
<path class="diagram-flow" d="M51 104 V128 M117 104 V128 M183 104 V128 M249 104 V128"/>
<rect class="diagram-operation" x="26" y="130" width="248" height="32" rx="4"/>
<text class="diagram-instruction" x="150" y="146">one +1 instruction</text>
<path class="diagram-flow" d="M51 164 V182 M46 176 L51 182 L56 176 M117 164 V182 M112 176 L117 182 L122 176 M183 164 V182 M178 176 L183 182 L188 176 M249 164 V182 M244 176 L249 182 L254 176"/>
<rect class="diagram-cell diagram-cell-active" x="24" y="190" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="90" y="190" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="156" y="190" width="54" height="44" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="222" y="190" width="54" height="44" rx="4"/>
<text class="diagram-value" x="51" y="212">5</text>
<text class="diagram-value" x="117" y="212">8</text>
<text class="diagram-value" x="183" y="212">3</text>
<text class="diagram-value" x="249" y="212">10</text>
<text class="diagram-label" x="18" y="277">The same add changes every lane.</text>
</svg>
</div>
</div>
<figcaption>Highlighted cells are processed by one add instruction. Four vector lanes are shown here.</figcaption>
</figure>

Imagine a person at a desk with a long tape passing over it. They can modify
the number in front of them, perhaps by adding 1. In a scalar loop, they
advance the tape and repeat that operation for the next number.

A vector operation gives them a wider desk with several numbers side by
side. There's still one person issuing the instruction, but linked pens
apply it to every number at once, as the diagram's wide desk shows.

This is SIMD: *single instruction, multiple data*. Each position is called a
*lane*. The instruction “add 1” acts on all the lanes together.

A processor holds these values in a vector register. Its width determines
how many numbers fit: a 128-bit register holds four 32-bit numbers, while a
256-bit register holds eight. “32-bit” describes the space one number takes.
Rake calls the row of values held together a *rack*.

### rake

Let's write an operation for a rack of numbers. We want to take each number's
square root, but negative numbers need a different result, so we'll give
those lanes zero. Here's the definition, which we'll read from top to bottom:

<!-- rake-check: verify x86-avx2 aarch64-neon wasm-simd128 -->
```rake
rake safe_root(values: f32s) -> f32s:
  tine #valid when values >= <0.0>

  through #valid else <0.0> into rooted:
    sqrt(values)

  sweep:
    | #valid => rooted
    | _      => <0.0>
```

The first line resembles a function declaration. `safe_root` is the name,
`values` is its input, and `f32s` means a rack of 32-bit floating-point
numbers. The arrow says the result is another rack of the same type.

The `rake` keyword tells the compiler that this definition will choose work
for individual lanes. Its body supplies that work. The next line tells us
which lanes to select.

### tine

A tine is a prong on a garden rake. In the language, a `tine` is a named
*mask*: a true-or-false choice for each lane. Read
`tine #valid when values >= <0.0>` as “the valid tine catches numbers that
are greater than or equal to zero”.

`<0.0>` is a *uniform*, one scalar value shared by every lane. The comparison
checks each input number against that same zero:

```text
values:   [16,   -4,    9,    1]
#valid:   [true, false, true, true]
```

`tine #valid` declares the mask's name, and `when values >= <0.0>` defines how
to calculate it. That follows the familiar distinction between a declaration,
which introduces a name, and a definition, which supplies its meaning. Here
both are on one line. The comparison computes the mask when the rake runs,
and the `#` marks the name we'll use to refer to it.

### through

Now that we've selected the valid lanes, we can pass them through a
calculation. Read `through #valid else <0.0> into rooted:` as “calculate the
valid lanes using the body below, give the other lanes zero, and call the
result `rooted`”. The body is `sqrt(values)`, so those valid lanes take
their square roots:

```text
values:   [16, -4, 9, 1]
rooted:   [ 4,  0, 3, 1]
```

Giving the result the name `rooted` is called *binding* a name to a value.
Binding is general programming terminology, used in languages such as
[OCaml](https://ocaml.org/docs/values-and-functions). It's the association
between a name and what that name means. Here `into rooted` introduces the
name, and the block defines its value through the calculation and the fallback.

We can now use `rooted` in a later block or in the sweep. A binding doesn't
require the compiler to store a temporary array: this value can stay in a
vector register. Any names introduced inside the through body are local to
that body, while `rooted` is available to the rest of the rake.

### sweep

We've calculated an intermediate rack. A `sweep` chooses the values that
leave the function. Read its arms in order: `| #valid => rooted` takes a
value from `rooted` wherever `#valid` holds. The final arm, `| _ => <0.0>`,
gives zero to any lane left over. The `_` means “everything else”.

The result is `[4, 0, 3, 1]`. Each value stays in its original lane, so a
sweep doesn't shuffle or compact the rack. `sweep:` is itself the rake's
result form, which is why it doesn't need a `return` keyword.

### Intermediate and final results

The example has zero in two places because it produces two results along
the way. The through block fills the inactive lanes of `rooted` with zero.
Then the sweep chooses the values to return. Its fallback is a separate
choice: change its last arm to `| _ => <-1.0>` and the final rack changes:

| Stage | Lane 0 | Lane 1 | Lane 2 | Lane 3 |
| --- | ---: | ---: | ---: | ---: |
| Input | 16 | −4 | 9 | 1 |
| `#valid` | true | false | true | true |
| `rooted` | 4 | 0 | 3 | 1 |
| Result | 4 | −1 | 3 | 1 |

The intermediate zero is still there, but the sweep never uses it. It takes
−1 for that lane instead. In the original version, choosing zero again is
redundant, so the compiler can remove the duplicate selection. If a sweep
uses the whole intermediate rack, the through fallback contributes to the
final result too.

Reading the definition in stages explains how the calculation depends on
its inputs. It doesn't specify a delayed execution model: the sweep isn't
a trigger for earlier work. The compiler can optimise these pure
calculations together. On WebAssembly this rake becomes a vector comparison,
a square root and a selection of results.

[Lesson 8](/docs/playground/#lesson-8) lets you change each fallback separately
and inspect the result. [Tines, through and sweeps](/docs/tines-and-through/)
defines the scope and masking rules.

## Characteristics

### Racks are vector values

`f32s` is a rack of `f32` values. On AVX2 it is one physical register holding
eight floats, and on NEON one holding four. On WebAssembly it is one `v128`
value holding four. Rake adheres to the virtual machine's fiction and doesn't
try to replace the runtime's physical register allocation. Its promise stays
the same: outside a `slow` block, rack work uses vector instructions wherever
the selected profile supports the operation, or the compiler refuses it.

### Scalars are marked

A value shared by every lane is a uniform, and angle brackets mark it where
it is declared and where it is used: `<dt: f32>` and `<dt>`. Every point where
a scalar becomes a rack is visible on the page, which makes the cost of
broadcasting easy to see.

### Masks choose lanes

There are no branches inside a rack. A tine such as `#valid` names a mask of
lanes, a `through` block computes under it, and a sweep picks each lane's
result by priority, ending in `_` so that every lane gets one. Lanes outside a
mask can't fail or raise a floating-point exception.

### Stages fuse

Once we've given an intermediate value a name, it can look as though we've
asked the processor to perform and store that step separately. *Fusion* is
the general compiler term for combining pieces of work so they can execute
together. Compilers may fuse loops, for example, to do their work in one
pass. Rake's fused bindings let it combine stages of a vector calculation.

In `| name <| expression`, the expression flows from right to left into its
name. This is still a binding. The leading `|` marks it as part of a fused
calculation, whose stages the compiler must keep together as a contiguous
sequence of vector instructions. Here a `crunch`, a function of racks that
doesn't need tines, calculates a new position in two stages:

```rake
crunch advance(positions: f32s, velocities: f32s, <dt: f32>) -> f32s:
  | step  <| velocities * <dt>
  | moved <| positions + step
  return moved
```

`step` is the velocity multiplied by the time interval. `moved` adds that
step to the position. The bindings make the calculation legible, while
allowing the compiler to combine the multiply and add into one instruction.
On AVX2 the code becomes one `vbroadcastss` to share `dt` across the lanes,
then one `vfmadd231ps`, a fused multiply-add. It rounds the combined
calculation once. Strict WebAssembly SIMD has no such instruction, so that
target keeps the multiply and add separate.

[Fused bindings](/docs/fused-bindings/) defines which rewrites Rake permits.
[LLVM's loop-fusion documentation](https://llvm.org/docs/LoopFusion.html)
describes another use of the same compiler term.

### Data lives in columns

A `stack` declares the columns of a structure of arrays, grouped by stored
type, and a traversal visits a `pack` of them a rack at a time. A column of
bytes stays one byte per record in memory and is widened only when the code
computes with it:

```rake
stack Particles {
  f32: position, velocity;
  u8: age;
}

run advance(particles: pack Particles, <count: i64>, <dt: f32>) -> f32:
  for particle in particles using f32s up to <count>:
    let age = to_f32(bitcast(i32s, widen(particle.age)))
    yield particle.position + particle.velocity * <dt> / (age + <1.0>)
```

The last partial rack of a traversal loads and stores only the records that
exist.

### Scalar code stays scalar

Programs need setup, records, state and calls to C as well as kernels. That
code is marked `slow`, and it can't hold a rack. Inside a run, `slow { ... }`
is a scoped escape for scalar loops, calls and state updates. It can produce a
scalar value, which becomes a rack only through an explicit broadcast:

```rake
run shift(values: []i32, out: mut []i32, <steps: i32>):
  let rack = values[<0>]
  let <offset: i32> = slow {
    total: i32 := 0
    for index from 0 up to steps:
      total <- total + index
    total
  }
  out[<0>] <- rack + <offset>
```

The loop computes a scalar offset. After `}`, one vector add applies it to
every lane. [The slow tier](/docs/slow-tier/#slow-blocks) defines the boundary.

### Every claim is checked

`rakec --verify-native` disassembles what it built and checks it against the
profile's rules: no hidden calls or stack work, one physical register per rack
on physical targets, and only the permitted WebAssembly virtual instructions
on `wasm-simd128`. A rack sine has no vector implementation yet, so the
compiler rejects `sin(values)` instead of calling a scalar library function
once per lane.

## Notation

Most of Rake reads like any expression language. These marks are its own:

| Mark | Example | Meaning |
| --- | --- | --- |
| `<...>` | `<dt>`, `<0.5>` | a uniform: one value shared by every lane |
| `#name` | `#valid` | a tine: a named mask of lanes |
| `\| name <\| e` | `\| moved <\| positions + step` | a fused stage: `e` flows into `name` |
| `\| #tine => e` | `\| #valid => rooted` | a sweep arm: the lanes of `#tine` take `e` |
| `:=` and `<-` | `total := <0.0>`, `total <- total + x` | a mutable location, and assigning to it |
| `~~` | `~~ a comment` | a comment to the end of the line |

If you know [Gleam's pipe operator](https://tour.gleam.run/everything/),
the flow marks may look familiar. In Rake, `<|` binds a stage from right to
left, while `<-` assigns to a mutable location. Neither creates a lazy
pipeline. The fused stages describe one computation for the compiler to
optimise.

Indentation shows structure, as in Python, and a line ending in `:` opens a
body. [The tutorial](/docs/playground/#reading-rake) explains each mark for
readers who come from C or Python.

## Targets

| Profile | Rack | Compiles |
| --- | --- | --- |
| `x86-avx2` | one 256-bit register, 8 `f32` lanes | crunches and rakes over `f32s`, as assembly |
| `aarch64-neon` | one 128-bit register, 4 `f32` lanes | crunches and rakes over `f32s`, as assembly |
| `wasm-simd128` | one `v128`, 4 `f32` lanes | float and integer racks, runs and whole programs, as C |

[Racks and targets](/docs/racks-and-targets/) lists what each profile
compiles, and [the roadmap](/docs/roadmap/) what comes next.

## Projects using Rake

<div class="project">
<img src="/images/the-loong-game.svg" alt="" width="1440" height="960" loading="lazy">
<div>
<h3>The Loong Game</h3>
<p>An open source bot and toolkit for the 2026 UNSW Battlecode tournament. A Rake profile for WebAssembly writes its room-count masks as C the tournament's judge accepts, at 437 points a turn against 435 for hand-written intrinsics.</p>
<ul>
<li><a href="https://over-yonder.tech/games/loong/the-choice/#rake-for-vector-kernels">The choice: Rake for vector kernels</a></li>
<li><a href="https://over-yonder.tech/games/loong/counting-room-faster/#keeping-it-vectorised">Counting room faster: keeping it vectorised</a></li>
</ul>
</div>
</div>

## Getting Rake

The compiler is written in OCaml, and its Nix development shell pins every
tool it uses:

```sh
git clone https://github.com/rakelang/rake
cd rake
nix develop --command dune build
nix develop --command dune exec rakec -- --interpret program.rk
```

[The rakec command](/docs/rakec/) lists its modes and options. Rake is
released under the MIT licence. This is a beta: the language and its binary
boundaries may still change between versions.

Slow blocks are available on `main` and in the playground, ahead of the next
tagged release. Rakes on `main` also end with `sweep:`, without `return`.
The changelog separates these changes from 0.4.0-beta.
