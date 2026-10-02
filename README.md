# rake-lang.org

The website for [Rake](https://github.com/rakelang/rake). Its pages are
generated: the front page from `src/index.md`, and the documentation from the
compiler repository's Markdown, listed with each page's address, title and
description in `src/pages.tsv`.

| Path | Holds |
| --- | --- |
| `src/pages.tsv` | every page: its URL, its source, its sidebar group, its JSON-LD type, title and description |
| `src/template.html` | the page layout |
| `src/index.md`, `src/docs.md`, `src/404.md` | the pages this repository writes |
| `src/static/styles/tokens.css` | the design tokens: colours, type and spacing |
| `src/static/styles/components.css` | every component the pages use |
| `src/static/images/` | the wallpaper, icons, social card and project images |
| `src/social-card.svg` | the source of `images/social-card.png` |
| `tools/site/` | `rake-site`, which builds and checks the site |
| `public/` | the built site, which is what gets deployed |

## Building

The site flake pins its Rust, Node, Tree-sitter and Emscripten tools. The
build enters that shell automatically, reads the compiler checkout in
`../rake` and the grammar in `../tree-sitter-rake`, and writes `public/`:

```sh
tools/build.sh
tools/build.sh check
```

`RAKE_DIR`, `RAKE_BRANCH` and `TREE_SITTER_RAKE_DIR` choose another compiler
checkout, the branch that GitHub links point at, and another grammar
checkout. Code is highlighted at build time by the Tree-sitter grammars, Rake's
with the grammar's own `queries/highlights.scm`, and the build fails if any
Rake block doesn't parse. Asset links carry a hash of the asset, so a changed
stylesheet or image gets a new URL.

For generator development, run `nix develop` and `cargo run --manifest-path
tools/site/Cargo.toml -- build` from the site root. `RAKE_SITE_DIR` selects the
root when invoking the generator from elsewhere. The build script sets it
explicitly, so a cached generator always operates on the requested checkout.
Use `--profile profiling` for profiling and
`--release` for the published build. Cargo output uses the host's target root
under `rake-lang-site`, or an explicitly announced cache-directory fallback.

`check` rebuilds into a temporary directory and requires `public/` to match
it. It then checks every page's title, description, canonical link, Open Graph
and Twitter tags, H1, JSON-LD and internal links and anchors, the sitemap and
`robots.txt`, that every page shows the release, and that every Rake block is
highlighted. The compiler's `tools/check_documentation_examples.sh` compiles
the Rake blocks in `src/`, and its `tools/check_website.sh` runs this check.

## Deploying

Commit `public/` with the sources it was built from, then deploy it to the
Cloudflare Pages project `rake-lang`:

```sh
nix run nixpkgs#wrangler -- pages deploy public --project-name=rake-lang --branch=main
```

The GitHub workflow runs the same command on a push to `main`. After
deploying, run over-yonder.tech's `tools/seo_audit.py` against the site.
