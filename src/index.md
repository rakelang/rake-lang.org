<div class="home-intro">
<img class="home-intro-image" src="/images/wallpaper.webp" alt="The Rake mark, a rake's head drawn as a grid of tines, above furrows of raked sand" width="1024" height="1536">
<div>
<h1>Rake</h1>
<p>What Rust does for safety with <code>unsafe {}</code>, Rake does for speed with <code>slow {}</code>.</p>
<p>Rust's type system and borrow checker enforce memory safety in safe code. An <code>unsafe</code> block marks operations whose safety the programmer must establish. Rake's compiler checks that vector calculations become vector instructions. You get that guarantee without needing to manually review the assembly that the compiler generated, because it does that for you. If it can't keep a calculation vectorised, compilation fails. A <code>slow { ... }</code> block makes an explicit place for scalar work, and vector code resumes after the closing brace.</p>
<p>Rake is a SIMD, or vector, programming language. It's built for calculations that apply the same operation to many numbers at once. Its current profiles target CPUs and WebAssembly. The planned GPU profiles will preserve parallel work across warp lanes and check execution costs against an explicit contract.</p>
<ul class="link-row">
<li><a class="link-button" href="/docs/">Documentation</a></li>
<li><a class="link-button" href="/docs/playground/">Tutorial</a></li>
<li><a class="link-button" href="https://github.com/rakelang/rake">Compiler on GitHub</a></li>
</ul>
</div>
</div>

## A first look

This demonstration processes a million floats in eight-lane racks, with
vector lowering checked by the compiler. The comparison starts with an
ordinary C loop:

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

The equivalent operation on a Rake rack is:

<!-- rake-check: verify x86-sse2 x86-avx2 x86-avx512 aarch64-neon wasm-simd128 -->
```rake
tine #valid(values: f32s) means values >= <0.0>

rake safe_root(values: f32s) -> f32s:
  through #valid(values) into rooted:
    sqrt(values)
  sweep:
    | #valid(values) => rooted
    | #valid(values) gaps => <0.0>
```

The native AVX2 demonstration puts this operation in a compiler-generated
loop over a million entries. Both Rake and optimized C took about 0.14 ms on
our Ryzen 7 5800X3D. C already vectorized the loop. Disabling C's vectorizer
made its scalar loop about 26 times slower. Those are measured build/input
comparisons, rather than a promise that Rake always beats C.
[The benchmark source and commands](https://github.com/rakelang/rake/tree/main/demo/safe-root)
include the flags and correctness checks, and generate both disassemblies.

Rake's execution model can be pictured in three steps: define the `tine`s,
`rake` data `through` them, then `sweep` the results. We'll read the program
in that order, starting with what vector processing means.

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

### Define the tines

Let's extend that first operation with a second case, so we can see how
tines combine. Positive numbers take their
square roots. Negative numbers take the square roots of their magnitudes,
then keep their negative signs: 16 becomes 4, and −4 becomes −2. Here's that
compound definition, which we'll read from top to bottom:

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

A tine is a prong on a garden rake. In the language, a `tine` is a labelled
*mask*: a true-or-false choice for each lane. The hash, `#`, resembles the
crossing lines and prongs of a rake. It also marks a label, another way to
think about referring to a mask. Read
`tine #nonnegative(values: f32s) means values >= <0.0>` as “the nonnegative tine catches
numbers that are greater than or equal to zero”. The second tine catches
the negative numbers.

`<0.0>` is a *uniform*, one scalar value shared by every lane. The angle
brackets visually stretch that value across the whole rack. Each comparison
checks every input number against the same zero:

```text
values:        [16,    -4,     9,    -1]
#nonnegative: [true, false,  true, false]
#negative:    [false, true, false,  true]
```

The tine's parameter, `values: f32s`, says that it takes a rack of 32-bit
floating-point numbers. `tine #nonnegative` declares its label, and
`means values >= <0.0>` defines how to calculate its mask. That follows the
familiar distinction between a declaration, which introduces an identifier,
and a definition, which supplies its meaning. Both are on one line here.

This definition sits outside the rake, so another rake can reuse it with
different data. It stores no global mask. The application
`#nonnegative(values)` computes the mask for the particular input.

### Rake the data

With the tines defined, the `rake signed_root` line introduces the operation
that will use them. `values` is its input, and the arrow says that the
result is another `f32s` rack. The `rake` keyword tells the compiler that
this definition will choose work for individual lanes. Its through blocks
supply that work, and its sweep chooses the result.

### through

Now that we've selected the valid lanes, we can pass them through a
calculation. Read `through #nonnegative(values) into positive_roots:` as
“calculate the nonnegative lanes using the body below, and bind those
results to `positive_roots`”. The body is `sqrt(values)`.
The second block calculates `-sqrt(-values)` under the negative tine:

```text
values:         [16, -4, 9, -1]
positive_roots: [ 4,  ·, 3,  ·]
negative_roots: [ ·, -2, ·, -1]
```

The dots mean undefined lanes. We haven't supplied an intermediate
fallback, so those lanes can't be read. The compiler checks every later
use against the mask where the value is defined.

Giving the result the name `positive_roots` is called *binding* a name to a value.
Binding is general programming terminology, used in languages such as
[OCaml](https://ocaml.org/docs/values-and-functions). It's the association
between a name and what that name means. Here `into positive_roots` introduces the
identifier, and the block defines its value through the masked calculation.

We can now use `positive_roots` under the same mask, or a narrower one, in
a later block or in the sweep. A binding doesn't
require the compiler to store a temporary array: this value can stay in a
vector register. Identifiers introduced inside a through body are local to
that body. Its result binding is available to the rest of the rake.

### sweep

We've calculated an intermediate rack. A `sweep` chooses the values that
leave the function. Read its arms in order: `| #nonnegative(values) => positive_roots`
takes the positive roots in the nonnegative lanes. The next arm takes the
negative roots in the negative lanes. The final arm uses
`(#nonnegative(values) or #negative(values)) gaps`: the gaps in the union
of both masks. It gives zero to any lane left over, including a NaN that
passed neither comparison. `gaps` is exact mask inversion.

The result is `[4, -2, 3, -1]`. Each value stays in its original lane, so a
sweep doesn't shuffle or compact the rack. `sweep:` is itself the rake's
result form, which is why it doesn't need a `return` keyword.

### Intermediate and final results

The sweep reads each partial result only in its defined lanes:

| Stage | Lane 0 | Lane 1 | Lane 2 | Lane 3 |
| --- | ---: | ---: | ---: | ---: |
| Input | 16 | −4 | 9 | −1 |
| `positive_roots` | 4 | undefined | 3 | undefined |
| `negative_roots` | undefined | −2 | undefined | −1 |
| Sweep result | 4 | −2 | 3 | −1 |

For a negative input, the sweep takes the value from `negative_roots`.
Trying to take it from `positive_roots` is a compile error. If a later
calculation needs the whole intermediate rack, add an explicit through
fallback such as `else <0.0>`. That makes every lane defined.

A sweep can also use a final `_` arm for unmatched lanes. It is optional
when the masks provably cover every lane, as these tines and their `gaps`
do. Separate numeric comparisons aren't assumed to cover all inputs:
NaNs fail both comparisons in this example.

Reading the definition in stages explains how the calculation depends on
its inputs. It doesn't specify a delayed execution model: the sweep isn't
a trigger for earlier work. The compiler can optimise these pure
calculations together. Both square roots become vector instructions. On
physical CPU profiles, the compiler supplies benign operands in their
inactive lanes. WebAssembly SIMD has no floating-point exception flags.

[Lesson 8](/docs/playground/#lesson-8) lets you try an undefined-lane read,
then add a fallback and inspect the result. [Tines, through and sweeps](/docs/tines-and-through/)
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

There are no branches inside a rack. A tine such as `#nonnegative` identifies a mask of
lanes, a `through` block computes under it, and a sweep picks each lane's
result by priority, with proven coverage or a final `_`. Lanes outside a
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
sequence of vector instructions. Here a `scratch`, a function of racks that
doesn't need tines, calculates a new position in two stages:

```rake
scratch advance(positions: f32s, velocities: f32s, <dt: f32>) -> f32s:
  | step  <| velocities * <dt>
  | moved <| positions + step
  moved
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

### Packs, racks and stacks

A scratch or rake operates on one rack. To describe the data it will work
on, we define a `pack`: one record, with its fields stored together. Here
one particle has a position, velocity and age:

```rake
pack Particles {
  f32: position, velocity;
  u8: age;
}
```

These fields use `f32` and `u8`, because each record stores one float or byte.
Their rack counterparts, `f32s` and `u8s`, describe values during vector
computation. Putting `f32s` in this declaration would confuse a stored
element with a processor-sized row of elements.

A `stack` is a collection of those packs transposed into columns. For 600
particles, it holds 600 positions together, 600 velocities together and
600 ages together. Each unsized column can be split into racks at the
target's SIMD width. Remember the hierarchy as "define our pack, then rack
'em and stack 'em".

<figure class="diagram">
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
<figcaption>The highlighted slice is one eight-lane AVX2 rack. The diagram shows the layout, not an automatic conversion from an array of records.</figcaption>
</figure>

A `run` walks the stack and feeds each rack into a scratch or rake. That's its
job beyond those two constructs: it handles the memory traversal, including
the final partial rack. This run feeds our 600 positions into `signed_root`:

<!-- rake-check: verify x86-sse2 x86-avx2 x86-avx512 aarch64-neon wasm-simd128 with 2 -->
```rake
pack Positions {
  f32: value;
}

run roots(positions: stack Positions, <count: i64>) -> f32:
  for row in positions using f32s up to <count>:
    yield signed_root(row.value)
```

`using f32s` selects the rack's element type. `<count>` is 600 for this stack.
It needn't be a power of two or a multiple of the rack width. The traversal
loads and stores only existing records in its final rack.

For the comparison at the top, `safe_root` replaces `signed_root` in this
same traversal, and the count is 1,000,000. AVX2 splits that column into
125,000 eight-lane racks. The benchmark also checks 1,000,003 entries, whose
last rack has three active lanes. The stored column contains a million
individual `f32` values, rather than a million-lane `f32s` value.

<figure class="diagram">
<svg viewBox="0 0 680 258" role="img" aria-labelledby="pack-width-title pack-width-description">
<title id="pack-width-title">600 float records divided into SIMD racks</title>
<desc id="pack-width-description">512-bit racks hold 16 floats, requiring 37 full racks and one half-full rack. AVX2 racks hold 8 floats, requiring 75 full racks. 128-bit racks hold 4 floats, requiring 150 full racks.</desc>
<text class="diagram-label" x="18" y="24">One column: 600 f32 records</text>
<rect class="diagram-cell diagram-cell-active" x="18" y="38" width="644" height="30" rx="4"/>
<text class="diagram-value" x="340" y="53">600 values in memory</text>
<text class="diagram-label" x="18" y="106">512-bit: 16 per rack</text>
<rect class="diagram-cell diagram-cell-active" x="246" y="84" width="300" height="34" rx="4"/>
<text class="diagram-value" x="396" y="101">37 full + 1 half rack</text>
<text class="diagram-label" x="18" y="157">AVX2: 8 per rack</text>
<rect class="diagram-cell diagram-cell-active" x="246" y="135" width="300" height="34" rx="4"/>
<text class="diagram-value" x="396" y="152">75 full racks</text>
<text class="diagram-label" x="18" y="208">128-bit: 4 per rack</text>
<rect class="diagram-cell diagram-cell-active" x="246" y="186" width="300" height="34" rx="4"/>
<text class="diagram-value" x="396" y="203">150 full racks</text>
<text class="diagram-label" x="18" y="247">Wider racks process more lanes per instruction.</text>
</svg>
<figcaption>The rack counts follow from register width. Native AVX-512 streams use the 512-bit row, AVX2 streams use the 256-bit row, and SSE2, NEON and WebAssembly use the 128-bit row.</figcaption>
</figure>

512-bit SIMD handles twice as many `f32` lanes per instruction as AVX2, and
four times as many as 128-bit SIMD. That describes the width of the work,
not a guaranteed speedup: memory bandwidth, instruction costs and processor
frequency also affect elapsed time. *WIP: work in progress.*

Our particle columns can use the same traversal. The age stays one byte per
record in memory. `widen` brings the current rack's ages into 32-bit lanes
before the calculation:

```rake
pack Particles {
  f32: position, velocity;
  u8: age;
}

run advance(particles: stack Particles, <count: i64>, <dt: f32>) -> f32:
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

The double squiggle, `~~`, is Rake's own line-comment marker. It evokes
furrows in raked sand, part of the language's desert theme. Lua's equivalent
is `--`. Rake borrows `let` bindings, expression-based conditionals and nested
`(* ... *)` block comments from OCaml.

We're still condensing this visual language into something “uniquely
comprehensible”. Function spellings such as `bit_and()` are temporary: they
keep bitwise operations separate from other operators while the symbology
settles. Any replacement will change the compiler, grammar and examples
together.

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
| `x86-sse2` | one 128-bit register, 4 `f32` lanes | scratches and rakes over `f32s`, as assembly |
| `x86-avx2` | one 256-bit register, 8 `f32` lanes | scratches and rakes over `f32s`, as assembly |
| `x86-avx512` | one 512-bit register, 16 `f32` lanes | scratches and rakes over `f32s`, as assembly |
| `aarch64-neon` | one 128-bit register, 4 `f32` lanes | scratches and rakes over `f32s`, as assembly |
| `wasm-simd128` | one `v128`, 4 `f32` lanes | float and integer racks, runs and whole programs, as C |

[Primitives, operations, and targets](/docs/primitives-operations-and-targets/) lists what each profile
compiles, and [the roadmap](/docs/roadmap/) what comes next.

The unreleased development compiler also combines native slow orchestration
with Rake-selected register kernels, through a limited scalar C boundary.
Typed C callbacks and process arguments are implemented there. Native runs
include the SSE2, AVX2, AVX-512 and NEON read-only `f32` stream subset.
General native runs remain WIP*. [The backend](/docs/backend/#whole-programs) explains
which parts Rake emits and which parts use a platform C compiler.

## GPU execution: the design

A CPU rack fits in one vector register. On an NVIDIA GPU, the planned rack
will span a warp of 32 threads instead. Each thread holds one lane's value.
Rake will check that the work stays mapped across those lanes, with no hidden
loop that processes the rack serially inside one thread.

<figure class="diagram diagram-gpu-rack">
<div class="diagram-scroll" tabindex="0" role="region" aria-label="GPU rack diagram, scroll horizontally on a narrow screen">
<svg viewBox="0 0 680 228" role="img" aria-labelledby="home-gpu-title home-gpu-description">
<title id="home-gpu-title">A proposed GPU rack maps values across a warp</title>
<desc id="home-gpu-description">32 values map to 32 thread lanes. Lanes zero, one, two, thirty and thirty-one are illustrated, with the intervening lanes omitted. Each participating thread applies plus one to its own value. Rake checks that mapping. Hardware schedules the warp.</desc>
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
<figcaption>Proposed GPU execution. Per-thread arithmetic is the parallel lowering here, not a CPU scalar fallback. GPU support isn't implemented yet.</figcaption>
</figure>

Masks can leave some lanes inactive because the input takes different paths.
The compiler can't promise that every lane always works, or choose the
hardware's warp schedule. It can preserve the declared control flow and
reject forbidden spills, reloads or helper calls in checked regions. Memory
address patterns and synchronisation scopes will remain explicit.

The first proposed profile, `nvidia-ptx-sm120`, will emit inspectable PTX,
then use a pinned NVIDIA toolchain to build a cubin ahead of time. NVIDIA
will allocate physical registers and schedule instructions. Rake will verify
the resulting device artifact and load those exact bytes, with no unverified
PTX JIT fallback. Later designs cover portable SPIR-V/Vulkan and a direct
physical backend for a documented ISA.

| Checkable contract | Needs execution evidence |
| --- | --- |
| lane mapping and permitted control flow | input-dependent inactive lanes and workload balance |
| no forbidden spills in strict regions | achieved occupancy and the best register policy |
| explicit lane-to-address patterns | memory transactions, caches and transfer costs |
| declared numerical and synchronisation rules | elapsed time and application throughput |

Keeping more values in registers can reduce the number of resident warps.
No-spill therefore doesn't mean fastest. Enough independent work and balanced
workloads still matter. [The GPU design](/docs/gpu/) defines the proposed
contract, and [the comparison with ISPC and Bend 2](/docs/comparisons/#gpu-execution-rake-ispc-and-bend)
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

### Syntax highlighting

The Tree-sitter grammar is available through these package managers. These
packages provide parsing and highlighting for editor integrations, rather
than the Rake compiler:

<ul class="link-row">
<li><a class="link-button" href="https://pypi.org/project/tree-sitter-rake/">PyPI · tree-sitter-rake</a></li>
<li><a class="link-button" href="https://www.npmjs.com/package/tree-sitter-rake">npm · tree-sitter-rake</a></li>
<li><a class="link-button" href="https://crates.io/crates/tree-sitter-rake">Cargo · tree-sitter-rake</a></li>
</ul>
