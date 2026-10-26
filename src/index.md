<div class="home-intro">
<img class="home-intro-image" src="/images/wallpaper.webp" alt="The Rake mark, a rake's head drawn as a grid of tines, above furrows of raked sand" width="1024" height="1536">
<div>
<h1>Rake</h1>
<p>What Rust does for safety with <code>unsafe {}</code>, Rake does for speed with <code>slow {}</code>.</p>
<p>Rust's type system and borrow checker enforce memory safety in safe code, and inside an <code>unsafe</code> block the programmer is responsible for it. Rake's compiler checks that vector calculations become vector instructions, reviewing the generated assembly for you, and compilation fails if it can't keep a calculation vectorised. A <code>slow { ... }</code> block makes an explicit place for scalar work, and vector code resumes after the closing brace.</p>
<p>Rake is a SIMD, or vector, programming language for calculations that apply the same operation to many numbers at once. Its current profiles target CPUs and WebAssembly, and the planned GPU profiles will preserve parallel work across warp lanes and check execution costs against an explicit contract.</p>
<ul class="link-row">
<li><a class="link-button" href="/docs/">Documentation</a></li>
<li><a class="link-button" href="/docs/playground/">Tutorial</a></li>
<li><a class="link-button" href="https://github.com/rakelang/rake">Compiler on GitHub</a></li>
</ul>
</div>
</div>

## Checked vectorisation

Suppose we want the square roots of a column of numbers, with zero for
negative inputs. An ordinary C loop is easy to read:

```c
#include <math.h>
#include <stddef.h>

void safe_root_c(const float *values, float *roots, size_t count)
{
    for (size_t i = 0; i < count; ++i) {
        if (values[i] >= 0.0f)
            roots[i] = sqrtf(values[i]);
        else
            roots[i] = 0.0f;
    }
}
```

GCC 14.3 turns this into a vector loop for AVX2. A similar loop with a
different maths function fares worse:

```c
void sin_array_scalar(const float *input, float *output, size_t count)
{
    for (size_t i = 0; i < count; ++i)
        output[i] = sinf(input[i]);
}
```

C still compiles it, but in this build the loop calls the scalar `sinf`
library function once per element. The build succeeds anyway, so only
vectorisation diagnostics or the assembly would show that the work became
scalar.

Rake makes the vector requirement part of compilation:

<!-- rake-check: reject "call to 'sin' is not supported by native scratch lowering" -->
```rake
scratch reject_sin(values: f32s) -> f32s:
  | result <| sin(values)
  result
```

```text
$ rakec --verify-native --target x86-avx2 demo/soa-proof/rake/reject_sin.rk
native lowering: call to 'sin' is not supported by native scratch lowering
```

The profile has no vector lowering for this operation, so Rake stops at the
line responsible, before the scalar loop reaches the application.

### Explicit SIMD

The usual next step is to write the vector operations explicitly. For the
square-root loop, C's AVX2 intrinsics expose the register width, lane mask,
loads and stores, and the final partial vector needs its own memory mask:

```c
#include <immintrin.h>
#include <stddef.h>

static __m256 safe_root_rack(__m256 value)
{
    const __m256 zero = _mm256_setzero_ps();
    const __m256 valid = _mm256_cmp_ps(value, zero, _CMP_GE_OQ);
    const __m256 input = _mm256_and_ps(valid, value);
    return _mm256_and_ps(valid, _mm256_sqrt_ps(input));
}

void safe_root_avx2(const float *values, float *roots, size_t count)
{
    size_t i = 0;
    for (; count - i >= 8; i += 8) {
        const __m256 value = _mm256_loadu_ps(values + i);
        _mm256_storeu_ps(roots + i, safe_root_rack(value));
    }
    if (i < count) {
        const __m256i lanes = _mm256_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7);
        const __m256i remaining = _mm256_set1_epi32((int)(count - i));
        const __m256i active = _mm256_cmpgt_epi32(remaining, lanes);
        const __m256 value = _mm256_maskload_ps(values + i, active);
        _mm256_maskstore_ps(roots + i, active, safe_root_rack(value));
    }
}
```

[ISPC](https://ispc.github.io/ispc.html) supplies a parallel loop and manages
its lanes, while its C-family syntax keeps the array indices and statements:

```c
export void safe_roots(uniform float values[],
                       uniform float roots[],
                       uniform int count)
{
    foreach (i = 0 ... count) {
        float value = values[i];
        roots[i] = value >= 0.0f ? sqrt(value) : 0.0f;
    }
}
```

In Rake, we describe the eligible lanes and their calculation. The profile
chooses the width, and the compiler handles the loads, stores and tail:

<!-- rake-check: verify x86-sse2 x86-avx2 x86-avx512 aarch64-neon wasm-simd128 -->
```rake
pack Samples {
  f32: value;
}

tine #valid(values: f32s) means values >= <0.0>

rake safe_root(values: f32s) -> f32s:
  through #valid(values) into rooted:
    sqrt(values)
  sweep:
    | #valid(values) => rooted
    | #valid(values) gaps => <0.0>

run roots(input: stack Samples, output: stack Samples) -> stack Samples:
  output with { value: safe_root(input.value) }
```

### A polynomial sine

Suppose our algorithm only needs the sine of angles between −0.5 and 0.5
radians. A ninth-degree polynomial over that interval can replace the general
library call, and because it needs only multiplies and adds, it passes Rake's
vector checks:

<!-- rake-check: verify x86-avx2 -->
```rake
scratch bounded_sine(values: f32s) -> f32s:
  let square = values * values
  let fifth = <0.0000027557319223985893> * square - <0.0001984126984126984>
  let fourth = fifth * square + <0.008333333333333333>
  let third = fourth * square - <0.16666666666666666>
  let second = third * square + <1.0>
  values * second
```

### Measured execution

These bars return to the square-root example, with all four versions
processing the same million-element input. C's automatic vectoriser already
handles this loop, so its two vector versions and Rake finish at similar times.

<figure class="diagram diagram-benchmark">
<div class="diagram-panels">
<div class="diagram-panel">
<h4>All four versions</h4>
<svg viewBox="0 0 320 260" role="img" aria-labelledby="benchmark-all-title benchmark-all-desc">
<title id="benchmark-all-title">Square roots over a million floats</title>
<desc id="benchmark-all-desc">Median time in milliseconds: scalar C 4.003, autovectorized C 0.147, C intrinsics 0.146 and Rake 0.147. All bars start at zero on the same scale.</desc>
<text class="diagram-label" x="112" y="22">Time (ms), smaller is faster</text>
<text class="diagram-label" x="8" y="65">Scalar C</text>
<rect class="diagram-operation" x="112" y="42" width="180.12" height="24" rx="3"/>
<text class="diagram-label" x="112" y="85">4.003 ms</text>
<text class="diagram-label" x="8" y="115">C auto</text>
<rect class="diagram-operation" x="112" y="92" width="6.60" height="24" rx="3"/>
<text class="diagram-label" x="128" y="111">0.147 ms</text>
<text class="diagram-label" x="8" y="165">C intrinsics</text>
<rect class="diagram-operation" x="112" y="142" width="6.59" height="24" rx="3"/>
<text class="diagram-label" x="128" y="161">0.146 ms</text>
<text class="diagram-label" x="8" y="215">Rake</text>
<rect class="diagram-operation" x="112" y="192" width="6.61" height="24" rx="3"/>
<text class="diagram-label" x="128" y="211">0.147 ms</text>
<path class="diagram-flow" d="M112 226 H292"/>
<text class="diagram-label" x="112" y="248">0</text>
<text class="diagram-label" x="270" y="248">4 ms</text>
</svg>
</div>
<div class="diagram-panel">
<h4>Vector versions, enlarged</h4>
<svg viewBox="0 0 320 260" role="img" aria-labelledby="benchmark-vector-title benchmark-vector-desc">
<title id="benchmark-vector-title">Enlarged view of the three vector timings</title>
<desc id="benchmark-vector-desc">On a separate zero-to-0.16 millisecond scale, autovectorized C takes 0.147 milliseconds, C intrinsics 0.146 and Rake 0.147.</desc>
<text class="diagram-label" x="112" y="22">Time (ms), smaller is faster</text>
<text class="diagram-label" x="8" y="90">C auto</text>
<rect class="diagram-operation" x="112" y="65" width="164.95" height="24" rx="3"/>
<text class="diagram-label" x="112" y="110">0.147 ms</text>
<text class="diagram-label" x="8" y="145">C intrinsics</text>
<rect class="diagram-operation" x="112" y="120" width="164.76" height="24" rx="3"/>
<text class="diagram-label" x="112" y="165">0.146 ms</text>
<text class="diagram-label" x="8" y="200">Rake</text>
<rect class="diagram-operation" x="112" y="175" width="165.30" height="24" rx="3"/>
<text class="diagram-label" x="112" y="220">0.147 ms</text>
<path class="diagram-flow" d="M112 226 H292"/>
<text class="diagram-label" x="112" y="248">0</text>
<text class="diagram-label" x="248" y="248">0.16 ms</text>
</svg>
</div>
</div>
<figcaption>Ryzen 7 5800X3D, AVX2, GCC 14.3, 5 October 2026, while other work kept its 16 threads busy. Medians of 31 samples, each averaging four passes, with execution order rotated. The scalar row disables C's vectoriser.</figcaption>
</figure>

[The benchmark source, flags and raw samples](https://github.com/rakelang/rake/tree/main/demo/safe-root)
include bitwise result comparisons, guard-page tail checks and disassemblies.

## The programming model

Rake has no branches inside a rack. Its execution model has three steps:
define the `tine`s, `rake` data `through` them, then `sweep` the results.

### One instruction, several numbers

<figure class="diagram">
<div class="diagram-panels">
<div class="diagram-panel">
<h4>Scalar: one number at a time</h4>
<svg viewBox="0 0 320 288" role="img" aria-labelledby="scalar-desk-title scalar-desk-description">
<title id="scalar-desk-title">One scalar add at a narrow desk</title>
<desc id="scalar-desk-description">The tape holds 4, 7, 2 and 9. Only 4 is on the narrow desk. One add instruction changes it to 5, leaving the other numbers unchanged. Advancing the tape brings the next number onto the desk.</desc>
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

Picture a person at a desk with a long tape passing over it. In a scalar
loop, they add 1 to the number in front of them, then advance the tape and
repeat. A vector operation gives them a wider desk with several numbers side
by side, and linked pens apply the one instruction to all of them at once.
This is SIMD, *single instruction, multiple data*, and each position is a
*lane*. A processor holds the values in a vector register, and its width sets
how many fit: a 128-bit register holds four 32-bit numbers, and a 256-bit
register holds eight. Rake
calls the row of values held together a *rack*.

### Tines

This rake takes the square root of each nonnegative number, and for a
negative number it takes the root of the magnitude and keeps the sign, so 16
becomes 4 and −4 becomes −2:

<!-- rake-check: verify x86-sse2 x86-avx2 x86-avx512 aarch64-neon wasm-simd128 -->
```rake
tine #nonnegative(values: f32s) means values >= <0.0>
tine #negative(values: f32s) means values < <0.0>

rake signed_root(values: f32s) -> f32s:
  through #nonnegative(values) into positive_roots:
    sqrt(values)

  through #negative(values) into negative_roots:
    -sqrt(-values)

  sweep:
    | #nonnegative(values) => positive_roots
    | #negative(values) => negative_roots
    | (#nonnegative(values) or #negative(values)) gaps => <0.0>
```

A `tine`, named after the prongs of a garden rake, is a labelled *mask*: a
true-or-false choice for each lane. Read the first line as “the nonnegative
tine catches numbers that are greater than or equal to zero”. `<0.0>` is a
*uniform*, one scalar value shared by every lane, and the angle brackets mark
every point where a scalar becomes a rack, so the cost of broadcasting is
always visible:

```text
values:        [16,    -4,     9,    -1]
#nonnegative: [true, false,  true, false]
#negative:    [false, true, false,  true]
```

Tines are defined outside the rake, so another rake can reuse them with
different data. They store no global mask: applying `#nonnegative(values)`
computes the mask for that particular input.

### Rakes and through blocks

The `rake` keyword tells the compiler that `signed_root` chooses work for
individual lanes. Its through blocks supply that work, and its sweep chooses
the result. Read `through #nonnegative(values) into positive_roots:` as
“calculate the nonnegative lanes using the body below, and bind those results
to `positive_roots`”:

```text
values:         [16, -4, 9, -1]
positive_roots: [ 4,  ·, 3,  ·]
negative_roots: [ ·, -2, ·, -1]
```

The dots are undefined lanes, and the compiler checks every later use of
`positive_roots` against the mask where it's defined. The binding doesn't need
a temporary array, so the value can stay in a vector register.

### Sweeps

A `sweep` chooses the values that leave the function, trying its arms in
order. The first two take each lane's root from the through block that
computed it. The last arm gives zero to the lanes in neither mask, found by
`gaps`, an exact mask inversion, so a NaN that passed neither comparison still
gets a value. The result is
`[4, -2, 3, -1]`, with each value still in its original lane, and because
`sweep:` is the rake's result form, it needs no `return` keyword.

Reading `positive_roots` in a negative lane would be a compile error. If a
later calculation needs a whole intermediate rack, a through fallback such as
`else <0.0>` defines every lane. A final `_` arm catches unmatched lanes, and
it's optional when the masks provably cover every lane, as these tines and
their `gaps` do. Lanes outside a mask can't fail or raise a floating-point
exception, because physical CPU profiles give inactive lanes benign operands
and WebAssembly SIMD has no floating-point exception flags.

[Lesson 8](/docs/playground/#lesson-8) lets you try an undefined-lane read,
then add a fallback and inspect the result. [Tines, through and sweeps](/docs/tines-and-through/)
defines the scope and masking rules.

## Characteristics

### Fused stages

*Fusion* is the general compiler term for combining pieces of work so they
execute together. In `| name <| expression`, the expression flows from right
to left into its name. It's still a binding, but the leading `|` marks it as a
fused stage, and the compiler keeps consecutive stages together as one
contiguous sequence of vector instructions. Here a `scratch`, a function of
racks that doesn't need tines, calculates a new position in two stages:

```rake
scratch advance(positions: f32s, velocities: f32s, <dt: f32>) -> f32s:
  | step  <| velocities * <dt>
  | moved <| positions + step
  moved
```

The names keep the calculation legible, while on AVX2 the code becomes one
`vbroadcastss` to share `dt` across the lanes and one `vfmadd231ps`, a fused
multiply-add that rounds the combined calculation once. Strict WebAssembly
SIMD has no such instruction, so that target keeps the multiply and add
separate. [Fused bindings](/docs/fused-bindings/) defines which rewrites Rake
permits.

### Packs, racks and stacks

A `pack` describes one record, here a particle with a position, velocity and
age:

```rake
pack Particles {
  f32: position, velocity;
  u8: age;
}
```

A `stack` stores many packs as columns, so 600 particles become 600 positions
together, 600 velocities together and 600 ages together, and each column
splits into racks at the target's SIMD width. Remember the hierarchy as
"define our pack, then rack 'em and stack 'em".

<figure class="diagram diagram-data-layout">
<div class="diagram-scroll" tabindex="0" role="region" aria-label="Particle storage diagram, scroll horizontally on a narrow screen">
<svg viewBox="0 0 680 250" role="img" aria-labelledby="data-layout-title data-layout-description">
<title id="data-layout-title">One pack and a stack of columnar records</title>
<desc id="data-layout-description">A particle pack has a position, velocity and age. A stack of 600 particles has a column of 600 positions, a column of 600 velocities and a column of 600 ages. A rack selects a SIMD-width slice of a column. The first eight positions form one AVX2 float rack.</desc>
<text class="diagram-label" x="18" y="23">pack: one particle</text>
<rect class="diagram-cell" x="18" y="37" width="202" height="39" rx="4"/>
<rect class="diagram-cell" x="236" y="37" width="202" height="39" rx="4"/>
<rect class="diagram-cell" x="454" y="37" width="202" height="39" rx="4"/>
<text class="diagram-value" x="119" y="57">position: f32</text>
<text class="diagram-value" x="337" y="57">velocity: f32</text>
<text class="diagram-value" x="555" y="57">age: u8</text>
<text class="diagram-label" x="18" y="111">stack: 600 particles, one column per field</text>
<text class="diagram-label" x="18" y="151">positions</text>
<rect class="diagram-cell diagram-cell-active" x="135" y="130" width="138" height="33" rx="4"/>
<rect class="diagram-cell" x="281" y="130" width="375" height="33" rx="4"/>
<text class="diagram-value" x="204" y="147">p₀ … p₇</text>
<text class="diagram-value" x="468" y="147">p₈ … p₅₉₉</text>
<text class="diagram-label" x="18" y="195">velocities</text>
<rect class="diagram-cell" x="135" y="174" width="521" height="33" rx="4"/>
<text class="diagram-value" x="395" y="191">v₀ … v₅₉₉</text>
<text class="diagram-label" x="18" y="239">ages</text>
<rect class="diagram-cell" x="135" y="218" width="521" height="27" rx="4"/>
<text class="diagram-value" x="395" y="232">a₀ … a₅₉₉</text>
</svg>
</div>
<figcaption>The highlighted slice is one eight-lane AVX2 rack in a stack's column layout. The caller supplies a separate column for each field.</figcaption>
</figure>

A `run` applies scratches and rakes to whole stacks. This one feeds a column
of positions into `signed_root` and returns the stack with the results:

<!-- rake-check: verify x86-sse2 x86-avx2 x86-avx512 aarch64-neon wasm-simd128 with 4 -->
```rake
pack Positions {
  f32: value;
}

run roots(positions: stack Positions) -> stack Positions:
  positions with { value: signed_root(positions.value) }
```

The stack carries its count, which needn't be a multiple of the rack width,
and the final rack loads and stores only the records that exist. Calling the
run moves the stack into it, so the run updates the column in place. A run can
also choose which records to touch: `where #young` after its result would
update only the young particles, while `compact` keeps only them, moved to the
front of each column, and shrinks the count to match:

<!-- rake-check: verify x86-sse2 x86-avx2 x86-avx512 aarch64-neon wasm-simd128 -->
```rake
pack Particles {
  f32: position, velocity;
  u8: age;
}

run young_only(particles: stack Particles) -> stack Particles:
  let age = widen(particles.age)
  tine #young means age < <2>
  compact particles where #young
```

Which is faster depends on how many particles survive, so the source chooses.

### Slow code

Programs need setup, records, state and calls to C as well as kernels. That
code is marked `slow`, and it can't hold a rack. Inside a run, `slow { ... }`
is a scoped escape for scalar work, and its scalar result becomes a rack only
through an explicit broadcast:

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

The loop computes a scalar offset, and after `}` one vector add applies it to
every lane. [The slow tier](/docs/slow-tier/#slow-blocks) defines the boundary.

### Verified output

`rakec --verify-native` disassembles what it built and checks it against the
profile's rules: no hidden calls or stack work, one physical register per rack
on physical targets, and only the permitted WebAssembly virtual instructions
on `wasm-simd128`.

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

`~~` evokes furrows in raked sand. Rake borrows `let` bindings,
expression-based conditionals and `(* ... *)` block comments from OCaml, and
indentation from Python, with a line ending in `:` opening a body.
[The tutorial](/docs/playground/#reading-rake) explains each mark for readers
who come from C or Python.

## Targets

| Profile | Rack | Compiles |
| --- | --- | --- |
| `x86-sse2` | one 128-bit register, 4 32-bit lanes | float racks and a 32-bit integer subset, as assembly |
| `x86-avx2` | one 256-bit register, 8 32-bit lanes | float racks and a 32-bit integer subset, as assembly |
| `x86-avx512` | one 512-bit register, 16 32-bit lanes | float racks and a 32-bit integer subset, as assembly |
| `aarch64-neon` | one 128-bit register, 4 32-bit lanes | float racks and a 32-bit integer subset, as assembly |
| `wasm-simd128` | one `v128`, 4 `f32` lanes | float and integer racks, runs and whole programs, as C |

The native profiles also compile stack runs over `f32`, `i32` and `u32`
columns, while general native runs are still work in progress.
[Primitives, operations, and targets](/docs/primitives-operations-and-targets/) lists what each profile
compiles, [the backend](/docs/backend/#whole-programs) explains which parts
of a program come from Rake and which use a platform C compiler, and
[the roadmap](/docs/roadmap/) covers what comes next.

`rakec --emit-c` writes C for your own toolchain, embedding the kernel
assembly on native targets and using SIMD intrinsics on WebAssembly. Rake can
call C functions and exchange records, unions and typed callbacks across the C
ABI, as [the compiler command](/docs/rakec/#c-output-and-c-interoperation)
explains.

## GPU execution: the design

On an NVIDIA GPU, the planned rack will span a warp of 32 threads, each
holding one lane's value, and Rake will check that no hidden loop processes
the rack serially inside one thread.

<figure class="diagram diagram-gpu-rack">
<div class="diagram-scroll" tabindex="0" role="region" aria-label="GPU rack diagram, scroll horizontally on a narrow screen">
<svg viewBox="0 0 680 228" role="img" aria-labelledby="home-gpu-title home-gpu-description">
<title id="home-gpu-title">A proposed GPU rack maps values across a warp</title>
<desc id="home-gpu-description">32 values map to 32 thread lanes. Lanes zero, one, two, thirty and thirty-one are illustrated, with the intervening lanes omitted. Each participating thread applies plus one to its own value, and Rake checks that mapping while hardware schedules the warp.</desc>
<text class="diagram-label" x="18" y="24">Column in memory: 32 values</text>
<rect class="diagram-cell diagram-cell-active" x="18" y="38" width="644" height="32" rx="4"/>
<text class="diagram-value" x="340" y="54">One value per thread lane</text>
<path class="diagram-flow" d="M68 76 V100 M176 76 V100 M284 76 V100 M392 76 V100 M500 76 V100 M610 76 V100"/>
<rect class="diagram-cell diagram-cell-active" x="18" y="106" width="100" height="38" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="126" y="106" width="100" height="38" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="234" y="106" width="100" height="38" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="342" y="106" width="100" height="38" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="450" y="106" width="100" height="38" rx="4"/>
<rect class="diagram-cell diagram-cell-active" x="558" y="106" width="104" height="38" rx="4"/>
<text class="diagram-value" x="68" y="125">0: +1</text>
<text class="diagram-value" x="176" y="125">1: +1</text>
<text class="diagram-value" x="284" y="125">2: +1</text>
<text class="diagram-value" x="392" y="125">…</text>
<text class="diagram-value" x="500" y="125">30: +1</text>
<text class="diagram-value" x="610" y="125">31: +1</text>
<text class="diagram-label" x="18" y="181">Rake checks lane mapping and permitted costs.</text>
<text class="diagram-label" x="18" y="213">Hardware schedules the warp.</text>
</svg>
</div>
<figcaption>GPU design: each thread performs one lane's arithmetic in parallel with the other threads of its warp.</figcaption>
</figure>

The first proposed profile, `nvidia-ptx-sm120`, will emit inspectable PTX and
build a cubin ahead of time with a pinned NVIDIA toolchain. Rake will verify
that device artifact and load those exact bytes, with no unverified PTX JIT
fallback. Some properties can be checked in the artifact, while others only
show up when it runs:

| Checkable contract | Needs execution evidence |
| --- | --- |
| lane mapping and permitted control flow | input-dependent inactive lanes and workload balance |
| no forbidden spills in strict regions | achieved occupancy and the best register policy |
| explicit lane-to-address patterns | memory transactions, caches and transfer costs |
| declared numerical and synchronisation rules | elapsed time and application throughput |

[The GPU design](/docs/gpu/) defines the proposed contract, including later
SPIR-V/Vulkan and direct physical backends, and
[the comparison with ISPC and Bend 2](/docs/comparisons/#gpu-execution-rake-ispc-and-bend)
separates lane-parallel kernels from parallel fork–join tasks.

## Projects using Rake

<div class="project">
<img src="/images/the-loong-game.svg" alt="" width="1440" height="960" loading="lazy">
<div>
<h3>The Loong Game</h3>
<p>An open source bot and toolkit for the 2026 UNSW Battlecode tournament. The Loong Game uses Rake to compile room-counting kernels to WebAssembly through C, at 437 points a turn against 435 for hand-written intrinsics.</p>
<ul>
<li><a href="https://over-yonder.tech/games/loong/the-choice/#rake-for-vector-kernels">The choice: Rake for vector kernels</a></li>
<li><a href="https://over-yonder.tech/games/loong/counting-room-faster/#keeping-it-vectorised">Counting room faster: keeping it vectorised</a></li>
</ul>
</div>
</div>

## Getting Rake

The compiler, `rakec`, is written in OCaml, and
[its repository](https://github.com/rakelang/rake) explains how to build it
in a pinned Nix development shell. [The rakec command](/docs/rakec/) lists its
modes and options. Rake is released under the MIT-SLOP licence, and it's pre-1.0,
so the language and its binary boundaries may still change between versions.

### Syntax highlighting

These Tree-sitter grammar packages provide parsing and highlighting for editor
integrations. They don't include the compiler:

<ul class="link-row">
<li><a class="link-button" href="https://pypi.org/project/tree-sitter-rake/">PyPI · tree-sitter-rake</a></li>
<li><a class="link-button" href="https://www.npmjs.com/package/tree-sitter-rake">npm · tree-sitter-rake</a></li>
<li><a class="link-button" href="https://crates.io/crates/tree-sitter-rake">Cargo · tree-sitter-rake</a></li>
</ul>
