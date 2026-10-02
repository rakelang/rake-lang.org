//! Builds rake-lang.org from the Rake documentation, and checks the result.
//!
//! `rake-site build` reads `src/pages.tsv`, renders each page's Markdown with
//! `src/template.html`, highlights its code with the Tree-sitter grammars and
//! writes the site to `public/`. `rake-site check` builds into a temporary
//! directory, requires `public/` to match it, and checks every page against
//! the search and link rules that the README lists.

use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

const SITE_URL: &str = "https://rake-lang.org";
const GITHUB: &str = "https://github.com/rakelang/rake";
const RAKE_HIGHLIGHTS: &str = include_str!(concat!(env!("TREE_SITTER_RAKE_QUERIES"), "/highlights.scm"));

/// Capture names the grammars use, each emitted as the class `tok-<name>`
/// with dots as hyphens. The site's design stylesheets colour them.
const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute", "boolean", "comment", "constant", "constant.builtin", "constructor", "delimiter",
    "embedded", "escape", "function", "function.builtin", "keyword", "label", "number", "operator", "operator.flow",
    "property", "punctuation.bracket", "punctuation.delimiter", "punctuation.special", "string",
    "string.special", "tag", "type", "type.builtin", "variable", "variable.builtin", "variable.parameter",
];

extern "C" {
    fn tree_sitter_rake() -> *const tree_sitter::ffi::TSLanguage;
}

struct Config {
    site: PathBuf,
    rake: PathBuf,
    branch: String,
    playground_assets: PathBuf,
}

#[derive(Clone, PartialEq)]
enum Kind {
    Home,
    DocsIndex,
    Doc,
    NotFound,
}

struct Page {
    url: String,
    source: PathBuf,
    /// The source's path inside the Rake repository, for resolving its links.
    rake_path: Option<String>,
    group: String,
    schema: String,
    title: String,
    description: String,
    kind: Kind,
}

struct Highlighters {
    rake: HighlightConfiguration,
    c: HighlightConfiguration,
    bash: HighlightConfiguration,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let config = match configure() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("rake-site: {error}");
            return ExitCode::FAILURE;
        }
    };
    let result = match arguments.first().map(String::as_str) {
        Some("build") => build(&config, &config.site.join("public")).map(|pages| {
            println!("rake-site: built {pages} pages into public/");
        }),
        Some("check") => check(&config),
        _ => Err("usage: rake-site build | rake-site check".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rake-site: {error}");
            ExitCode::FAILURE
        }
    }
}

fn configure() -> Result<Config, String> {
    let site = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let site = fs::canonicalize(&site).map_err(|e| format!("site root {}: {e}", site.display()))?;
    let rake = std::env::var("RAKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| site.join("../rake"));
    let rake = fs::canonicalize(&rake).map_err(|e| format!("Rake checkout {}: {e} (set RAKE_DIR)", rake.display()))?;
    let branch = std::env::var("RAKE_BRANCH").unwrap_or_else(|_| "main".to_string());
    let playground_assets = std::env::var("PLAYGROUND_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| site.join(".build/playground"));
    Ok(Config { site, rake, branch, playground_assets })
}

// ─── Pages ──────────────────────────────────────────────────────────────

fn read_pages(config: &Config) -> Result<Vec<Page>, String> {
    let table = read(&config.site.join("src/pages.tsv"))?;
    let mut pages = Vec::new();
    for (number, line) in table.lines().enumerate().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [url, source, group, schema, title, description] = fields[..] else {
            return Err(format!("src/pages.tsv:{}: expected 6 tab-separated fields", number + 1));
        };
        let (source_path, rake_path) = match source.split_once(':') {
            Some(("site", path)) => (config.site.join(path), None),
            Some(("rake", path)) => (config.rake.join(path), Some(path.to_string())),
            _ => return Err(format!("src/pages.tsv:{}: source is site:path or rake:path", number + 1)),
        };
        let kind = match url {
            "/" => Kind::Home,
            "/docs/" => Kind::DocsIndex,
            "/404.html" => Kind::NotFound,
            _ => Kind::Doc,
        };
        pages.push(Page {
            url: url.to_string(),
            source: source_path,
            rake_path,
            group: group.to_string(),
            schema: schema.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            kind,
        });
    }
    Ok(pages)
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn version(config: &Config) -> Result<String, String> {
    let source = read(&config.rake.join("src/lib/version.ml"))?;
    source
        .lines()
        .find_map(|line| line.strip_prefix("let value = \"").and_then(|rest| rest.strip_suffix('"')))
        .map(str::to_string)
        .ok_or_else(|| "src/lib/version.ml has no `let value = \"...\"`".to_string())
}

// ─── Build ──────────────────────────────────────────────────────────────

fn build(config: &Config, out: &Path) -> Result<usize, String> {
    let pages = read_pages(config)?;
    let version = version(config)?;
    let template = read(&config.site.join("src/template.html"))?;
    let highlighters = highlighters()?;
    let routes: BTreeMap<String, String> = pages
        .iter()
        .filter_map(|page| page.rake_path.clone().map(|path| (path, page.url.clone())))
        .collect();

    if out.exists() {
        fs::remove_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    let mut assets = copy_static(&config.site.join("src/static"), out)?;
    assets.extend(copy_static(&config.playground_assets, out)?);

    for page in &pages {
        let markdown = read(&page.source)?;
        let rendered = render(&markdown, page, config, &routes, &highlighters)?;
        let html = fill(&template, page, &pages, &rendered, &version, &assets)?;
        let path = output_path(out, &page.url);
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&path, html).map_err(|e| format!("{}: {e}", path.display()))?;
    }

    let mut sitemap = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    for page in pages.iter().filter(|page| page.kind != Kind::NotFound) {
        writeln!(sitemap, "  <url><loc>{SITE_URL}{}</loc></url>", page.url).unwrap();
    }
    sitemap.push_str("</urlset>\n");
    fs::write(out.join("sitemap.xml"), sitemap).map_err(|e| e.to_string())?;
    fs::write(out.join("robots.txt"), format!("User-agent: *\nAllow: /\n\nSitemap: {SITE_URL}/sitemap.xml\n"))
        .map_err(|e| e.to_string())?;
    Ok(pages.len())
}

fn output_path(out: &Path, url: &str) -> PathBuf {
    if url.ends_with('/') {
        out.join(url.trim_start_matches('/')).join("index.html")
    } else {
        out.join(url.trim_start_matches('/'))
    }
}

/// Copies `src/static` into the output and returns each asset's cache-busting
/// version, a hash of its bytes.
fn copy_static(from: &Path, out: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut assets = BTreeMap::new();
    let mut stack = vec![from.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory).map_err(|e| format!("{}: {e}", directory.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path.strip_prefix(from).unwrap().to_string_lossy().replace('\\', "/");
            let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let target = out.join(&relative);
            fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::write(&target, &bytes).map_err(|e| e.to_string())?;
            assets.insert(relative, format!("{:08x}", fnv1a(&bytes) as u32));
        }
    }
    Ok(assets)
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| (hash ^ *byte as u64).wrapping_mul(0x100000001b3))
}

// ─── Markdown ───────────────────────────────────────────────────────────

struct Rendered {
    body: String,
    /// The page's second-level headings, for its contents list.
    sections: Vec<(String, String)>,
}

fn render(
    markdown: &str,
    page: &Page,
    config: &Config,
    routes: &BTreeMap<String, String>,
    highlighters: &Highlighters,
) -> Result<Rendered, String> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let events: Vec<Event> = Parser::new_ext(markdown, options).collect();
    let mut output: Vec<Event> = Vec::new();
    let mut sections = Vec::new();
    let mut used_ids = BTreeSet::new();
    let mut index = 0;
    while index < events.len() {
        match &events[index] {
            // The playground uses one source marker per lesson. Other HTML
            // comments are documentation-checker annotations, not page copy.
            Event::Html(html) | Event::InlineHtml(html) if html.trim() == "<!-- playground-starter -->" => {
                output.push(Event::Html(CowStr::from("<span data-playground-starter hidden></span>\n")));
            }
            Event::Html(html) | Event::InlineHtml(html) if html.trim_start().starts_with("<!--") => {}
            Event::Start(Tag::Heading { level, .. }) => {
                let level = *level;
                let mut inner = Vec::new();
                index += 1;
                while !matches!(events[index], Event::End(TagEnd::Heading(_))) {
                    inner.push(rewrite_link(events[index].clone(), page, config, routes)?);
                    index += 1;
                }
                let text = plain_text(&inner);
                let id = unique_id(&slug(&text), &mut used_ids);
                let mut html = String::new();
                pulldown_cmark::html::push_html(&mut html, inner.into_iter());
                let tag = heading_tag(level);
                if level == HeadingLevel::H2 {
                    sections.push((id.clone(), html.clone()));
                }
                output.push(Event::Html(CowStr::from(format!("<{tag} id=\"{id}\">{html}</{tag}>\n"))));
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info.split(|c: char| c == ',' || c.is_whitespace()).next().unwrap_or("").to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                let mut code = String::new();
                index += 1;
                while !matches!(events[index], Event::End(TagEnd::CodeBlock)) {
                    if let Event::Text(text) = &events[index] {
                        code.push_str(text);
                    }
                    index += 1;
                }
                let html = code_block(&language, &code, highlighters)
                    .map_err(|error| format!("{}: {error}", page.source.display()))?;
                output.push(Event::Html(CowStr::from(html)));
            }
            Event::Start(Tag::Table(_)) => {
                output.push(Event::Html(CowStr::from("<div class=\"table-scroll\">")));
                output.push(events[index].clone());
            }
            Event::End(TagEnd::Table) => {
                output.push(events[index].clone());
                output.push(Event::Html(CowStr::from("</div>")));
            }
            event => output.push(rewrite_link(event.clone(), page, config, routes)?),
        }
        index += 1;
    }
    let mut body = String::new();
    pulldown_cmark::html::push_html(&mut body, output.into_iter());
    // Column alignment becomes a class, so the stylesheet owns it.
    for side in ["left", "center", "right"] {
        body = body.replace(&format!(" style=\"text-align: {side}\""), &format!(" class=\"align-{side}\""));
    }
    Ok(Rendered { body, sections })
}

fn heading_tag(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "h1",
        HeadingLevel::H2 => "h2",
        HeadingLevel::H3 => "h3",
        HeadingLevel::H4 => "h4",
        HeadingLevel::H5 => "h5",
        HeadingLevel::H6 => "h6",
    }
}

fn plain_text(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Text(text) | Event::Code(text) => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

/// GitHub's heading anchors: lower case, spaces as hyphens, other punctuation
/// dropped, so the documentation's links work on GitHub and here alike.
fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

fn unique_id(base: &str, used: &mut BTreeSet<String>) -> String {
    let mut id = base.to_string();
    let mut n = 1;
    while used.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    used.insert(id.clone());
    id
}

/// A link between documentation pages becomes a link between site pages, and
/// a link to another file in the Rake repository goes to it on GitHub.
fn rewrite_link<'a>(event: Event<'a>, page: &Page, config: &Config, routes: &BTreeMap<String, String>) -> Result<Event<'a>, String> {
    let Event::Start(Tag::Link { link_type, dest_url, title, id }) = event else {
        return Ok(event);
    };
    let destination = dest_url.to_string();
    let rewritten = if destination.starts_with("http://") || destination.starts_with("https://") || destination.starts_with('#') || destination.starts_with("mailto:") {
        destination
    } else if let Some(source) = &page.rake_path {
        let (path, fragment) = match destination.split_once('#') {
            Some((path, fragment)) => (path, format!("#{fragment}")),
            None => (destination.as_str(), String::new()),
        };
        let base = Path::new(source).parent().unwrap_or(Path::new(""));
        let target = normalise(&base.join(path));
        if let Some(url) = routes.get(&target) {
            format!("{url}{fragment}")
        } else {
            let on_disk = config.rake.join(&target);
            if !on_disk.exists() {
                return Err(format!("{}: link to {destination} resolves to {target}, which doesn't exist", page.source.display()));
            }
            let kind = if on_disk.is_dir() { "tree" } else { "blob" };
            format!("{GITHUB}/{kind}/{}/{target}{fragment}", config.branch)
        }
    } else {
        destination
    };
    Ok(Event::Start(Tag::Link { link_type, dest_url: CowStr::from(rewritten), title, id }))
}

fn normalise(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => {}
        }
    }
    parts.join("/")
}

// ─── Highlighting ───────────────────────────────────────────────────────

fn highlighters() -> Result<Highlighters, String> {
    let configure = |language: tree_sitter::Language, name: &str, query: &str| {
        let mut config = HighlightConfiguration::new(language, name, query, "", "").map_err(|e| format!("{name} highlights: {e}"))?;
        config.configure(HIGHLIGHT_NAMES);
        Ok::<_, String>(config)
    };
    let rake = unsafe { tree_sitter::Language::from_raw(tree_sitter_rake()) };
    Ok(Highlighters {
        rake: configure(rake, "rake", RAKE_HIGHLIGHTS)?,
        c: configure(tree_sitter_c::LANGUAGE.into(), "c", tree_sitter_c::HIGHLIGHT_QUERY)?,
        bash: configure(tree_sitter_bash::LANGUAGE.into(), "bash", tree_sitter_bash::HIGHLIGHT_QUERY)?,
    })
}

fn code_block(language: &str, code: &str, highlighters: &Highlighters) -> Result<String, String> {
    let (config, class) = match language {
        "rake" => (Some(&highlighters.rake), "rake"),
        "c" => (Some(&highlighters.c), "c"),
        "sh" | "bash" | "console" => (Some(&highlighters.bash), "sh"),
        _ => (None, "text"),
    };
    let body = match config {
        Some(config) => {
            if language == "rake" {
                require_parse(code)?;
            }
            highlight(config, code)?
        }
        None => escape(code),
    };
    Ok(format!("<pre class=\"code code-{class}\"><code>{body}</code></pre>\n"))
}

/// Every Rake block parses without an error node, so its highlighting is the
/// grammar's reading of a whole program.
fn require_parse(code: &str) -> Result<(), String> {
    let mut parser = tree_sitter::Parser::new();
    let language = unsafe { tree_sitter::Language::from_raw(tree_sitter_rake()) };
    parser.set_language(&language).map_err(|e| e.to_string())?;
    let tree = parser.parse(code, None).ok_or("Tree-sitter returned no tree")?;
    if tree.root_node().has_error() {
        let first = code.lines().next().unwrap_or("");
        return Err(format!("the Tree-sitter grammar can't parse the Rake block starting `{first}`"));
    }
    Ok(())
}

fn highlight(config: &HighlightConfiguration, code: &str) -> Result<String, String> {
    let mut highlighter = Highlighter::new();
    let events = highlighter.highlight(config, code.as_bytes(), None, |_| None).map_err(|e| e.to_string())?;
    let mut html = String::new();
    for event in events {
        match event.map_err(|e| e.to_string())? {
            HighlightEvent::HighlightStart(h) => {
                write!(html, "<span class=\"tok-{}\">", HIGHLIGHT_NAMES[h.0].replace('.', "-")).unwrap();
            }
            HighlightEvent::Source { start, end } => html.push_str(&escape(&code[start..end])),
            HighlightEvent::HighlightEnd => html.push_str("</span>"),
        }
    }
    Ok(html)
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            c => escaped.push(c),
        }
    }
    escaped
}

// ─── Template ───────────────────────────────────────────────────────────

fn fill(
    template: &str,
    page: &Page,
    pages: &[Page],
    rendered: &Rendered,
    version: &str,
    assets: &BTreeMap<String, String>,
) -> Result<String, String> {
    let canonical = format!("{SITE_URL}{}", page.url);
    let mut body = rendered.body.clone();
    if page.kind == Kind::DocsIndex {
        body.push_str(&docs_index(pages));
    }
    let (layout, sidebar, contents) = match page.kind {
        Kind::Doc if page.url == "/docs/playground/" => ("layout-playground", String::new(), String::new()),
        Kind::Doc | Kind::DocsIndex => ("layout-docs", docs_sidebar(pages, page), contents_list(&rendered.sections)),
        Kind::Home => ("layout-home", String::new(), String::new()),
        Kind::NotFound => ("layout-plain", String::new(), String::new()),
    };
    let robots = if page.kind == Kind::NotFound { "<meta name=\"robots\" content=\"noindex\">\n  " } else { "" };
    let schema = schema_json(page, &canonical, version);
    let (body, scripts) = if page.url == "/docs/playground/" {
        let (heading, reference) = body
            .split_once("</h1>\n")
            .ok_or("the playground page needs one H1 before the application")?;
        // Lesson fragments select application state. Keep their anchors at
        // the editor, rather than scrolling to the reference below it.
        let lesson_anchors: String = rendered.sections.iter().filter_map(|(_, title)| {
            let (number, _) = title.split_once(". ")?;
            let number = number.parse::<usize>().ok()?;
            Some(format!("<span id=\"lesson-{number}\" hidden></span>\n"))
        }).collect();
        (
            format!("{heading}</h1>\n{lesson_anchors}{}\n<article id=\"playground-reference\" class=\"playground-reference\">{reference}</article>", playground(assets)?),
            format!("<script type=\"module\" src=\"{}\"></script>", asset_url(assets, "scripts/playground.js")?),
        )
    } else {
        (body, String::new())
    };
    let mut html = template
        .replace("{{title}}", &escape(&page.title))
        .replace("{{description}}", &escape(&page.description))
        .replace("{{canonical}}", &canonical)
        .replace("{{robots}}", robots)
        .replace("{{schema}}", &schema)
        .replace("{{layout}}", layout)
        .replace("{{sidebar}}", &sidebar)
        .replace("{{contents}}", &contents)
        .replace("{{body}}", &body)
        .replace("{{version}}", version)
        .replace("{{github}}", GITHUB);
    html = html.replace("{{scripts}}", &scripts);
    while let Some(start) = html.find("{{asset:") {
        let end = html[start..].find("}}").ok_or("unterminated {{asset:")? + start;
        let name = &html[start + 8..end];
        let hash = assets.get(name).ok_or_else(|| format!("template asks for missing asset {name}"))?;
        html.replace_range(start..end + 2, &format!("/{name}?v={hash}"));
    }
    if html.contains("{{") {
        return Err(format!("{}: unfilled template placeholder", page.url));
    }
    Ok(html)
}

fn asset_url(assets: &BTreeMap<String, String>, name: &str) -> Result<String, String> {
    let hash = assets.get(name).ok_or_else(|| format!("playground asks for missing asset {name}"))?;
    Ok(format!("/{name}?v={hash}"))
}

fn playground(assets: &BTreeMap<String, String>) -> Result<String, String> {
    Ok(format!(r#"<section class="playground-app" data-rake-playground
  data-worker="{}"
  data-compiler="{}"
  data-tree-sitter="{}"
  data-rake-language="{}"
  data-rake-highlights="{}">
  <header class="playground-heading">
    <div>
      <p class="playground-progress" data-lesson-number>Lesson 1 of 12</p>
      <p class="playground-lesson-title" data-lesson-title>A program</p>
    </div>
    <nav class="playground-steps" aria-label="Tutorial lessons">
      <button type="button" data-previous>Previous</button>
      <button type="button" data-next>Next</button>
    </nav>
  </header>
  <div class="playground-grid">
    <section class="playground-lesson" aria-label="Lesson">
      <div data-lesson-text></div>
      <p class="playground-keyboard"><kbd>Ctrl</kbd>/<kbd>⌘</kbd> + <kbd>Enter</kbd> runs the program.</p>
    </section>
    <section class="playground-workbench" aria-label="Rake editor">
      <div class="playground-editor-shell">
        <pre class="playground-highlighting" aria-hidden="true"><code data-highlighting></code></pre>
        <textarea data-editor aria-label="Rake source code" aria-describedby="playground-status" autocomplete="off" autocapitalize="off" spellcheck="false"></textarea>
      </div>
      <div class="playground-controls">
        <button class="playground-run" type="button" data-run>Run</button>
        <label>Target
          <select data-target>
            <option value="wasm-simd128">wasm-simd128</option>
            <option value="x86-sse2">x86-sse2</option>
            <option value="x86-avx2">x86-avx2</option>
            <option value="x86-avx512">x86-avx512</option>
            <option value="aarch64-neon">aarch64-neon</option>
          </select>
        </label>
        <button type="button" data-reset>Reset</button>
        <label class="playground-check"><input type="checkbox" data-ligatures checked> Ligatures</label>
      </div>
      <p class="playground-target-note" data-target-note></p>
      <div class="playground-output">
        <div class="playground-tabs" role="tablist" aria-label="Compiler output">
          <button id="playground-tab-result" type="button" role="tab" aria-selected="true" aria-controls="playground-panel-result" data-tab="result">Result</button>
          <button id="playground-tab-lanes" type="button" role="tab" aria-selected="false" aria-controls="playground-panel-lanes" tabindex="-1" data-tab="lanes">Lanes</button>
          <button id="playground-tab-code" type="button" role="tab" aria-selected="false" aria-controls="playground-panel-code" tabindex="-1" data-tab="code">Code</button>
          <button id="playground-tab-messages" type="button" role="tab" aria-selected="false" aria-controls="playground-panel-messages" tabindex="-1" data-tab="messages">Messages</button>
        </div>
        <div id="playground-panel-result" class="playground-panel" role="tabpanel" aria-labelledby="playground-tab-result" data-panel="result"></div>
        <div id="playground-panel-lanes" class="playground-panel" role="tabpanel" aria-labelledby="playground-tab-lanes" data-panel="lanes" hidden></div>
        <pre id="playground-panel-code" class="playground-panel playground-code" role="tabpanel" aria-labelledby="playground-tab-code" data-panel="code" hidden></pre>
        <div id="playground-panel-messages" class="playground-panel" role="tabpanel" aria-labelledby="playground-tab-messages" data-panel="messages" hidden></div>
      </div>
      <p class="playground-status" id="playground-status" role="status" data-status>Loading the compiler…</p>
    </section>
  </div>
</section>"#,
        asset_url(assets, "scripts/playground-worker.js")?,
        asset_url(assets, "scripts/rake-compiler.js")?,
        asset_url(assets, "scripts/tree-sitter.wasm")?,
        asset_url(assets, "scripts/tree-sitter-rake.wasm")?,
        asset_url(assets, "scripts/rake-highlights.scm")?,
    ))
}

fn schema_json(page: &Page, canonical: &str, version: &str) -> String {
    if page.schema == "-" {
        return String::new();
    }
    let publisher = r#"{"@type":"Organization","name":"over|yonder","url":"https://over-yonder.tech/"}"#;
    let json = match page.schema.as_str() {
        "SoftwareSourceCode" => format!(
            r#"{{"@context":"https://schema.org","@type":"SoftwareSourceCode","name":"Rake","url":"{canonical}","description":"{}","codeRepository":"{GITHUB}","programmingLanguage":"Rake","version":"{version}","license":"https://opensource.org/licenses/MIT","publisher":{publisher}}}"#,
            json_escape(&page.description)
        ),
        other => format!(
            r#"{{"@context":"https://schema.org","@type":"{other}","headline":"{}","name":"{}","description":"{}","url":"{canonical}","isPartOf":{{"@type":"WebSite","name":"Rake","url":"{SITE_URL}/"}},"publisher":{publisher}}}"#,
            json_escape(&page.title),
            json_escape(&page.title),
            json_escape(&page.description)
        ),
    };
    format!("<script type=\"application/ld+json\">{json}</script>")
}

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn docs_sidebar(pages: &[Page], current: &Page) -> String {
    let mut html = String::from("<nav class=\"docs-nav\" aria-label=\"Documentation\">\n");
    let mut group = "";
    for page in pages.iter().filter(|page| matches!(page.kind, Kind::Doc | Kind::DocsIndex)) {
        if page.group != group {
            if !group.is_empty() {
                html.push_str("</ul>\n");
            }
            group = &page.group;
            write!(html, "<p class=\"docs-nav-group\">{}</p>\n<ul>\n", escape(group)).unwrap();
        }
        let current_attribute = if page.url == current.url { " aria-current=\"page\"" } else { "" };
        write!(html, "<li><a href=\"{}\"{current_attribute}>{}</a></li>\n", page.url, escape(short_title(page))).unwrap();
    }
    html.push_str("</ul>\n</nav>");
    html
}

/// The sidebar shows a page's title without the site name after it.
fn short_title(page: &Page) -> &str {
    page.title.split(" | ").next().unwrap_or(&page.title)
}

fn contents_list(sections: &[(String, String)]) -> String {
    if sections.len() < 2 {
        return String::new();
    }
    let mut html = String::from("<nav class=\"page-contents\" aria-label=\"On this page\">\n<p class=\"page-contents-title\">On this page</p>\n<ul>\n");
    for (id, title) in sections {
        write!(html, "<li><a href=\"#{id}\">{title}</a></li>\n").unwrap();
    }
    html.push_str("</ul>\n</nav>");
    html
}

fn docs_index(pages: &[Page]) -> String {
    let mut html = String::new();
    let mut group = "";
    for page in pages.iter().filter(|page| page.kind == Kind::Doc) {
        if page.group != group {
            if !group.is_empty() {
                html.push_str("</dl>\n");
            }
            group = &page.group;
            write!(html, "<h2 id=\"{}\">{}</h2>\n<dl class=\"docs-list\">\n", slug(group), escape(group)).unwrap();
        }
        write!(html, "<dt><a href=\"{}\">{}</a></dt>\n<dd>{}</dd>\n", page.url, escape(short_title(page)), escape(&page.description)).unwrap();
    }
    if !group.is_empty() {
        html.push_str("</dl>\n");
    }
    html
}

// ─── Check ──────────────────────────────────────────────────────────────

fn check(config: &Config) -> Result<(), String> {
    let public = config.site.join("public");
    let fresh = std::env::temp_dir().join(format!("rake-site-check-{}", std::process::id()));
    let built = build(config, &fresh);
    let comparison = built.and_then(|_| compare_trees(&fresh, &public));
    let _ = fs::remove_dir_all(&fresh);
    comparison?;

    let pages = read_pages(config)?;
    let version = version(config)?;
    let mut problems = Vec::new();
    let mut titles = BTreeMap::new();
    let mut descriptions = BTreeMap::new();
    let mut ids: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut documents = BTreeMap::new();
    for page in &pages {
        let html = read(&output_path(&public, &page.url))?;
        ids.insert(page.url.clone(), attribute_values(&html, "id").into_iter().collect());
        documents.insert(page.url.clone(), html);
    }
    for page in &pages {
        let html = &documents[&page.url];
        let mut problem = |message: String| problems.push(format!("{}: {message}", page.url));
        if !html.contains(&format!("Rake {version}")) {
            problem(format!("doesn't show the release, Rake {version}"));
        }
        for (number, block) in html.split("<pre class=\"code code-rake\">").skip(1).enumerate() {
            let block = block.split("</pre>").next().unwrap_or("");
            if !block.contains("class=\"tok-keyword\"") {
                problem(format!("Rake block {} has no highlighted keywords", number + 1));
            }
        }
        for (target, fragment) in internal_links(html) {
            let resolved = if target.is_empty() { page.url.clone() } else { target.clone() };
            if !resolved.starts_with('/') {
                problem(format!("relative link {target}"));
                continue;
            }
            let file = output_path(&public, &resolved);
            if !file.is_file() {
                let as_directory = public.join(resolved.trim_start_matches('/')).join("index.html");
                problem(if as_directory.is_file() { format!("link {resolved} needs its trailing slash") } else { format!("broken link {resolved}") });
                continue;
            }
            if let Some(fragment) = fragment {
                if ids.get(&resolved).is_some_and(|known| !known.contains(&fragment)) {
                    problem(format!("link {resolved}#{fragment} has no such anchor"));
                }
            }
        }
        if page.kind == Kind::NotFound {
            if !html.contains("<meta name=\"robots\" content=\"noindex\">") {
                problem("the 404 page needs noindex".to_string());
            }
            continue;
        }
        let title = between(html, "<title>", "</title>").unwrap_or_default();
        let description = meta(html, "name", "description").unwrap_or_default();
        let length = |text: &str| unescape(text).chars().count();
        if !(15..=60).contains(&length(&title)) {
            problem(format!("title is {} characters, not 15 to 60: {title}", length(&title)));
        }
        if !(120..=155).contains(&length(&description)) {
            problem(format!("description is {} characters, not 120 to 155", length(&description)));
        }
        if let Some(other) = titles.insert(title.clone(), page.url.clone()) {
            problem(format!("shares its title with {other}"));
        }
        if let Some(other) = descriptions.insert(description.clone(), page.url.clone()) {
            problem(format!("shares its description with {other}"));
        }
        let canonical = format!("{SITE_URL}{}", page.url);
        let required = [
            ("<html lang=\"en\">".to_string(), "lang"),
            ("<meta charset=\"utf-8\">".to_string(), "charset"),
            ("name=\"viewport\"".to_string(), "viewport"),
            ("rel=\"icon\"".to_string(), "favicon"),
            (format!("<link rel=\"canonical\" href=\"{canonical}\">"), "canonical"),
            (format!("<meta property=\"og:url\" content=\"{canonical}\">"), "og:url"),
            (format!("<meta property=\"og:title\" content=\"{title}\">"), "og:title"),
            (format!("<meta property=\"og:description\" content=\"{description}\">"), "og:description"),
            ("<meta property=\"og:type\"".to_string(), "og:type"),
            ("<meta property=\"og:image\" content=\"https://".to_string(), "og:image"),
            ("<meta name=\"twitter:card\"".to_string(), "twitter:card"),
        ];
        for (literal, name) in required {
            if !html.contains(&literal) {
                problem(format!("missing or wrong {name}"));
            }
        }
        if html.matches("<h1").count() != 1 {
            problem(format!("has {} H1 headings, not one", html.matches("<h1").count()));
        }
        if page.schema != "-" && !html.contains(&format!("\"@type\":\"{}\"", page.schema)) {
            problem(format!("missing JSON-LD {}", page.schema));
        }
        if html.contains("noindex") {
            problem("an indexable page carries noindex".to_string());
        }
        if html.contains("style=\"") || html.contains("<style") {
            problem("styles belong in the design stylesheets".to_string());
        }
    }
    let sitemap = read(&public.join("sitemap.xml"))?;
    let listed: BTreeSet<String> = sitemap.split("<loc>").skip(1).filter_map(|s| s.split("</loc>").next()).map(str::to_string).collect();
    let expected: BTreeSet<String> = pages.iter().filter(|p| p.kind != Kind::NotFound).map(|p| format!("{SITE_URL}{}", p.url)).collect();
    if listed != expected {
        problems.push("sitemap.xml doesn't list exactly the indexable pages".to_string());
    }
    if !read(&public.join("robots.txt"))?.contains(&format!("Sitemap: {SITE_URL}/sitemap.xml")) {
        problems.push("robots.txt doesn't name the sitemap".to_string());
    }
    if problems.is_empty() {
        println!("rake-site: public/ is current, and its {} pages pass the search and link checks", pages.len());
        Ok(())
    } else {
        Err(format!("{} problems:\n  {}", problems.len(), problems.join("\n  ")))
    }
}

fn compare_trees(fresh: &Path, public: &Path) -> Result<(), String> {
    let files = |root: &Path| -> Result<BTreeMap<String, Vec<u8>>, String> {
        let mut found = BTreeMap::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(directory) = stack.pop() {
            for entry in fs::read_dir(&directory).map_err(|e| format!("{}: {e}", directory.display()))? {
                let path = entry.map_err(|e| e.to_string())?.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let relative = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                    found.insert(relative, fs::read(&path).map_err(|e| e.to_string())?);
                }
            }
        }
        Ok(found)
    };
    let (expected, actual) = (files(fresh)?, files(public)?);
    let mut stale: Vec<String> = Vec::new();
    for (name, bytes) in &expected {
        match actual.get(name) {
            None => stale.push(format!("public/{name} is missing")),
            Some(other) if other != bytes => stale.push(format!("public/{name} is out of date")),
            _ => {}
        }
    }
    for name in actual.keys().filter(|name| !expected.contains_key(*name)) {
        stale.push(format!("public/{name} isn't produced by the build"));
    }
    if stale.is_empty() {
        Ok(())
    } else {
        Err(format!("public/ doesn't match a fresh build; run tools/build.sh:\n  {}", stale.join("\n  ")))
    }
}

fn attribute_values(html: &str, attribute: &str) -> Vec<String> {
    let needle = format!(" {attribute}=\"");
    html.match_indices(&needle)
        .filter_map(|(at, _)| html[at + needle.len()..].split('"').next())
        .map(|value| value.to_string())
        .collect()
}

/// Every link to this site: its path, and its fragment if it has one.
fn internal_links(html: &str) -> Vec<(String, Option<String>)> {
    let mut links = Vec::new();
    for attribute in ["href", "src"] {
        for value in attribute_values(html, attribute) {
            if value.contains("://") || value.starts_with("mailto:") || value.starts_with("data:") {
                continue;
            }
            let value = value.split('?').next().unwrap_or("").to_string();
            let (path, fragment) = match value.split_once('#') {
                Some((path, fragment)) => (path.to_string(), Some(fragment.to_string())),
                None => (value, None),
            };
            links.push((path, fragment));
        }
    }
    links
}

fn between(html: &str, start: &str, end: &str) -> Option<String> {
    let from = html.find(start)? + start.len();
    let to = html[from..].find(end)? + from;
    Some(html[from..to].to_string())
}

fn meta(html: &str, key: &str, name: &str) -> Option<String> {
    let needle = format!("<meta {key}=\"{name}\" content=\"");
    let from = html.find(&needle)? + needle.len();
    html[from..].split('"').next().map(str::to_string)
}

fn unescape(text: &str) -> String {
    text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
}
