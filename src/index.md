<div class="home-intro">
<img class="home-intro-image" src="/images/wallpaper.webp" alt="The Rake mark, a rake's head drawn as a grid of tines, above furrows of raked sand" width="1024" height="1536">
<div>
<h1>Rake</h1>
<p>What Rust does for safety with <code>unsafe {}</code>, Rake does for speed with <code>slow {}</code>.</p>
<p>Rust's type system and borrow checker enforce memory safety in safe code, with <code>unsafe</code> marking operations whose safety the programmer must establish. Rake's compiler enforces SIMD lowering for rack work: vector kernels compile to the selected target's vector instructions, or compilation fails. Enter <code>slow { ... }</code> for scalar work, then return to vector mode at the closing brace.</p>
<p>Rake is a programming language for SIMD kernels, the inner loops that apply one operation to many numbers at once. Each rack is one vector register on a physical target, or one vector value in the WebAssembly virtual machine.</p>
<ul class="link-row">
<li><a class="link-button" href="/docs/">Documentation</a></li>
<li><a class="link-button" href="/docs/playground/">Tutorial</a></li>
<li><a class="link-button" href="https://github.com/rakelang/rake">Compiler on GitHub</a></li>
</ul>
</div>
</div>

## A first look

Rake's execution model can be pictured in a sentence: “`rake` data `through`
`tine`s, then `sweep` them into the result.” The words describe how values
move through a vector computation.

### One instruction, several numbers

Imagine a person at a desk with a long tape passing over it. In scalar code,
they read one number, add 1, then advance the tape to the next number. A vector
operation gives them a wider desk with several numbers side by side. They
still issue one instruction, but linked pens apply it to every number at once:

```text
scalar add 1:   [4]             → [5]
vector add 1:   [4, 7, 2, 9]    → [5, 8, 3, 10]
```

Each position is a *lane*. A 32-bit number occupies 32 bits, not 32 lanes.
A 128-bit vector can hold four 32-bit numbers, while a 256-bit vector can
hold eight. There is one instruction stream, rather than a separate program
running at each seat.

Rake calls one vector's worth of values a *rack*. A *pack* holds columns of
records, and a traversal visits those columns a rack at a time. The selected
target profile fixes the rack's width. The pack's length needn't be a power
of two or a multiple of that width: the traversal handles its last partial
rack without reading or writing beyond the records that exist.

### Rake data through tines

This program takes `sqrt(x)` when `x` is non-negative and gives zero
elsewhere. `safe_root` describes the operation on a rack. `roots` applies it
to a pack of seven numbers, including a partial rack on WebAssembly:

<!-- rake-check: run 12 -->
```rake
stack Samples {
  f32: value;
}

rake safe_root(values: f32s) -> f32s:
  tine #valid when values >= <0.0>

  through #valid else <0.0> into rooted:
    sqrt(values)

  sweep:
    | #valid => rooted
    | _      => <0.0>

run roots(input: pack Samples, <count: i64>) -> f32:
  for rack in input using f32s up to <count>:
    yield safe_root(rack.value)

slow main() -> i32:
  values: [7]f32 := [16.0, -4.0, 9.0, 1.0, 0.0, 25.0, -1.0]
  rooted: [7]f32 := [0.0; 7]
  roots(Samples { value: values }, <7>, rooted)
  return i32(rooted[0] + rooted[2] + rooted[5])
```

`rakec --interpret` prints 12, which is 4 + 3 + 5. Read `safe_root` in
three parts:

1. `tine #valid` names a mask, like the prongs of a rake catching selected
   values. Its comparison computes one true-or-false result per lane. `#valid`
   is the name we use to refer to that mask.
2. `through #valid` computes square roots in the selected lanes and binds the
   intermediate rack as `rooted`. Its `else <0.0>` fills the other lanes of
   that rack. Names declared inside the block stay inside it; `rooted` is
   available to subsequent blocks and the sweep.
3. `sweep:` gives the function's result. Each lane takes the first matching
   arm, with `_` supplying the value for any lane left over. Values keep
   their lane positions.

### Why are there two zeros?

The `through` fallback belongs to the intermediate rack. The sweep's fallback
belongs to the final result. They can differ. Change only the sweep's last
arm to `| _ => <-1.0>` and the first rack looks like this:

| Stage | Lane 0 | Lane 1 | Lane 2 | Lane 3 |
| --- | ---: | ---: | ---: | ---: |
| Input | 16 | −4 | 9 | 1 |
| `#valid` | true | false | true | true |
| `rooted` | 4 | 0 | 3 | 1 |
| Result | 4 | −1 | 3 | 1 |

In the original version, the sweep reads `rooted` only where `#valid` holds.
Its other lanes are unused, so the two zero selections are redundant in
this example. The compiler folds them into one. Another sweep can read the
whole intermediate rack, making the `through` fallback matter.

These are pure computations, with no side effects. They aren't lazy steps
waiting for a sweep to trigger execution. The compiler sees their data flow
together and chooses the instructions that implement it. On WebAssembly,
`safe_root` becomes a vector comparison, square root and select. A sweep
selects values in place; it doesn't scatter, compact or rearrange lanes.

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

`| name <| expression` is a stage of one fused computation, read from right
to left. The names are for the reader. The compiler sees one graph and may
compile it as the cheapest instructions it finds, such as a fused multiply-add:

```rake
crunch advance(positions: f32s, velocities: f32s, <dt: f32>) -> f32s:
  | step  <| velocities * <dt>
  | moved <| positions + step
  return moved
```

On AVX2 that is one `vbroadcastss` and one `vfmadd231ps`.

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
