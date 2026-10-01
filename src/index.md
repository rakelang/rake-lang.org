<div class="home-intro">
<img class="home-intro-image" src="/images/wallpaper.webp" alt="The Rake mark, a rake's head drawn as a grid of tines, above furrows of raked sand" width="1024" height="1536">
<div>
<h1>Rake</h1>
<p>Rake is a programming language for SIMD kernels, the inner loops that apply one operation to many numbers at once. Its values are racks, each one vector register of the machine, and the compiler keeps every rack in its register or refuses the program.</p>
<ul class="link-row">
<li><a class="link-button" href="/docs/">Documentation</a></li>
<li><a class="link-button" href="/docs/playground/">Tutorial</a></li>
<li><a class="link-button" href="https://github.com/rakelang/rake">Compiler on GitHub</a></li>
</ul>
</div>
</div>

## A first look

Here is a whole program. A rake called `safe_root` takes the square root of
the lanes that hold a non-negative number and gives zero elsewhere. A run
called `roots` walks an array four floats at a time, and `main`, ordinary
scalar code, fills the array and adds three of the results:

```rake
rake safe_root(values: f32s) -> f32s:
  tine #valid when values >= <0.0>

  through #valid else <0.0> into rooted:
    sqrt(values)

  return sweep:
    | #valid => rooted
    | _      => <0.0>

run roots(x: []f32, out: mut []f32, <n: i32>):
  for <i: i32> from <0> up to <n> by <4>:
    out[<i>] <- safe_root(x[<i>])

slow main() -> i32:
  values: [8]f32 := [16.0, -4.0, 9.0, 1.0, 0.0, 25.0, -1.0, 4.0]
  rooted: [8]f32 := [0.0; 8]
  roots(values, rooted, <8>)
  return i32(rooted[0] + rooted[2] + rooted[5])
```

`rakec --interpret` prints 12, which is 4 + 3 + 5. Compiled for
WebAssembly, `safe_root` becomes a comparison, a square root and a select on
whole vector registers, with no branch for any lane.

## Characteristics

### Racks are registers

`f32s` is a rack of `f32` values, one vector register wide. On AVX2 it holds
eight floats, and on NEON and WebAssembly four. The source never states the
width, so one program compiles for each machine, and the compiler fails
rather than split a rack across registers, keep it in memory or compute it a
lane at a time.

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
code is marked `slow`, and it can't hold a rack. It reaches vector code only
through calls whose uniform arguments are marked, so the guarantees above
hold for the whole program.

### Every claim is checked

`rakec --verify-native` disassembles what it built and checks it against the
profile's rules: no calls, no stack, every rack in one register, and nothing
outside the profile's list of instructions. A rack sine has no vector implementation
yet, so the compiler rejects `sin(values)` instead of calling a scalar
library function once per lane.

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
git clone --branch wasm-simd128 https://github.com/rakelang/rake
cd rake
nix develop --command dune build
nix develop --command dune exec rakec -- --interpret program.rk
```

[The rakec command](/docs/rakec/) lists its modes and options. Rake is
released under the MIT licence. This is a beta: the language and its binary
boundaries may still change between versions.
