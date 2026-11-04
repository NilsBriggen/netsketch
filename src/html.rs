//! A tiny HTML reader for the built-in browser: headings, paragraphs, links, rules and images.
//! shortcut: no tables, lists or CSS beyond body colour/centering — enough for classroom pages.

#[derive(Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub href: Option<String>,
    pub bold: bool,
}

#[derive(Debug, PartialEq)]
pub enum Block {
    Heading(u8, String),
    Text(Vec<Span>),
    Rule,
    Image(String),
}

#[derive(Debug, Default)]
pub struct Page {
    pub title: String,
    pub bg: Option<[u8; 3]>,
    pub center: bool,
    pub blocks: Vec<Block>,
}

fn decode(s: &str) -> String {
    s.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

fn attr(tag: &str, name: &str) -> Option<String> {
    // Case-insensitive search on the bytes themselves (lowercasing can change byte positions), and only
    // whole attribute names (not "data-href" for "href").
    let pat = format!("{name}=");
    let b = tag.as_bytes();
    let at = b
        .windows(pat.len())
        .enumerate()
        .find(|(i, w)| w.eq_ignore_ascii_case(pat.as_bytes()) && (*i == 0 || b[i - 1].is_ascii_whitespace()))
        .map(|(i, _)| i)?
        + pat.len();
    let rest = &tag[at..];
    let v = match rest.chars().next()? {
        q @ ('"' | '\'') => rest[1..].split(q).next()?,
        _ => rest.split(|c: char| c.is_whitespace() || c == '>').next()?,
    };
    Some(decode(v))
}

fn hex(c: &str) -> Option<[u8; 3]> {
    let c = c.trim().trim_start_matches('#');
    if c.len() != 6 || !c.is_ascii() {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&c[i..i + 2], 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

pub fn parse(html: &str) -> Page {
    let mut page = Page::default();
    if !html.contains('<') {
        page.blocks.push(Block::Text(vec![Span { text: html.trim().to_string(), href: None, bold: false }]));
        return page;
    }
    let mut spans: Vec<Span> = vec![];
    let (mut href, mut bold, mut in_title, mut skip, mut heading) = (None::<String>, false, false, false, None::<u8>);
    let flush = |spans: &mut Vec<Span>, heading: Option<u8>, blocks: &mut Vec<Block>| {
        if spans.iter().all(|s| s.text.trim().is_empty()) {
            spans.clear();
            return;
        }
        if let Some(l) = heading {
            blocks.push(Block::Heading(l, spans.iter().map(|s| s.text.as_str()).collect::<String>().trim().to_string()));
            spans.clear();
        } else {
            blocks.push(Block::Text(std::mem::take(spans)));
        }
    };
    let mut rest = html;
    while !rest.is_empty() {
        let lt = rest.find('<').unwrap_or(rest.len());
        let raw = &rest[..lt];
        if !raw.is_empty() {
            let words = decode(raw).split_whitespace().collect::<Vec<_>>().join(" ");
            if in_title {
                page.title += &words;
            } else if !skip && !words.is_empty() {
                let lead = if raw.starts_with(char::is_whitespace) && !spans.is_empty() { " " } else { "" };
                let tail = if raw.ends_with(char::is_whitespace) { " " } else { "" };
                spans.push(Span { text: format!("{lead}{words}{tail}"), href: href.clone(), bold: bold || heading.is_some() });
            }
        }
        if lt >= rest.len() {
            break;
        }
        // Comments can hold '>' and whole tags: skip them in one go.
        if rest[lt..].starts_with("<!--") {
            rest = rest[lt..].find("-->").map(|e| &rest[lt + e + 3..]).unwrap_or("");
            continue;
        }
        // A '<' that never closes is just text ("price < 5 €").
        let Some(gt) = rest[lt..].find('>').map(|g| lt + g) else {
            let words = decode(&rest[lt..]).split_whitespace().collect::<Vec<_>>().join(" ");
            if !skip && !words.is_empty() {
                let lead = if spans.last().is_some_and(|s| !s.text.ends_with(' ')) { " " } else { "" };
                spans.push(Span { text: format!("{lead}{words}"), href: href.clone(), bold });
            }
            break;
        };
        let tag = &rest[lt + 1..gt];
        rest = &rest[gt + 1..];
        let closing = tag.starts_with('/');
        let name = tag.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("").to_lowercase();
        match name.as_str() {
            // Only the page title (pictures inside can have their own).
            "title" => in_title = !closing && page.title.is_empty(),
            // Scripts and styles are raw text up to their closing tag, even if they contain '<'.
            "script" | "style" if !closing => {
                let close = rest.match_indices("</").map(|(i, _)| i).find(|i| rest.get(i + 2..i + 2 + name.len()).is_some_and(|t| t.eq_ignore_ascii_case(&name)));
                rest = close.and_then(|i| rest[i..].find('>').map(|g| &rest[i + g + 1..])).unwrap_or("");
            }
            "svg" | "template" => skip = !closing,
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                flush(&mut spans, heading, &mut page.blocks);
                heading = (!closing).then(|| name[1..].parse().unwrap_or(1));
            }
            "p" | "div" | "br" | "li" | "tr" | "ul" | "ol" | "table" | "center" | "body" | "html" => {
                flush(&mut spans, heading, &mut page.blocks);
                if name == "center" && !closing {
                    page.center = true;
                }
                if name == "body" && !closing {
                    page.bg = attr(tag, "bgcolor").and_then(|c| hex(&c));
                    page.center |= attr(tag, "style").is_some_and(|s| s.replace(' ', "").contains("text-align:center"));
                }
            }
            "hr" => {
                flush(&mut spans, heading, &mut page.blocks);
                page.blocks.push(Block::Rule);
            }
            "img" => {
                flush(&mut spans, heading, &mut page.blocks);
                page.blocks.push(Block::Image(attr(tag, "alt").filter(|a| !a.is_empty()).or_else(|| attr(tag, "src")).unwrap_or_default()));
            }
            "a" => href = if closing { None } else { attr(tag, "href") },
            "b" | "strong" => bold = !closing,
            _ => {}
        }
    }
    flush(&mut spans, heading, &mut page.blocks);
    page
}

/// Where a link on page `here` ("host/dir/page.html", no scheme) leads.
pub fn join_url(here: &str, href: &str) -> String {
    if href.contains("://") {
        return href.to_string();
    }
    if let Some(other) = href.strip_prefix("//") {
        return other.to_string();
    }
    let host = here.split('/').next().unwrap_or("");
    let mut parts: Vec<&str> = if href.starts_with('/') { vec![] } else { here.split('/').skip(1).collect() };
    // The current page's own name isn't a folder.
    if !href.starts_with('/') {
        parts.pop();
    }
    for seg in href.trim_start_matches('/').split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    format!("{host}/{}", parts.join("/"))
}

/// Page as plain text, for the command line.
pub fn plain(html: &str) -> String {
    let p = parse(html);
    let mut out: Vec<String> = vec![];
    if !p.title.is_empty() {
        out.push(format!("[{}]", p.title));
    }
    for b in p.blocks {
        match b {
            Block::Heading(_, t) => out.push(t.to_uppercase()),
            Block::Text(s) => out.push(s.iter().map(|s| s.text.as_str()).collect::<String>().trim().to_string()),
            Block::Rule => out.push("----".into()),
            Block::Image(a) => out.push(format!("[image: {a}]")),
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_classroom_page() {
        let p = parse(r##"<html><head><title>Bienvenue !</title></head>
<body bgcolor="#ccddff" style="font-family:Verdana; text-align:center;"><h1>Sécurité</h1>
<p>Bienvenue sur <b>FILIUS</b>! Voir <a href="page2.html">la suite</a> &amp; plus.</p><hr><img src="logo.png"></body></html>"##);
        assert_eq!((p.title.as_str(), p.bg, p.center), ("Bienvenue !", Some([0xcc, 0xdd, 0xff]), true));
        assert_eq!(p.blocks[0], Block::Heading(1, "Sécurité".into()));
        let Block::Text(spans) = &p.blocks[1] else { panic!("{:?}", p.blocks) };
        assert!(spans.iter().any(|s| s.bold && s.text.contains("FILIUS")));
        assert!(spans.iter().any(|s| s.href.as_deref() == Some("page2.html") && s.text == "la suite"));
        assert!(spans.last().unwrap().text.contains("& plus."));
        assert_eq!(p.blocks[2..], [Block::Rule, Block::Image("logo.png".into())]);
        assert_eq!(plain("just text"), "just text");
        // Real-world pages: comments with tags inside, styles holding tags and inline graphics are left out.
        assert_eq!(join_url("site.test/dir/index.html", "page2.html"), "site.test/dir/page2.html");
        assert_eq!(join_url("site.test/dir/sub/a.html", "../x.html"), "site.test/dir/x.html");
        assert_eq!(join_url("site.test/dir/a.html", "/top.html"), "site.test/top.html");
        assert_eq!(join_url("site.test", "page2.html"), "site.test/page2.html");
        assert_eq!(join_url("a.test/x", "//b.test/y"), "b.test/y");
        // Odd input never crashes: an unclosed '<', non-ASCII colours, attributes with Unicode before them.
        assert_eq!(plain("Größe < 5 m²"), "Größe < 5 m²");
        let _ = parse("<body bgcolor=\"#a€bc\"><a title=\"İİİİ\" href=\"x.html\">x</a> <img data-src=\"y\" src=\"z.png\">");
        assert!(matches!(&parse("<img data-src=\"y\" src=\"z.png\">").blocks[..], [Block::Image(s)] if s == "z.png"));
        assert_eq!(plain("<style>a{b:url('<svg><x></x></svg>')}</style><p>ok</p><script>if (a<b) x()</SCRIPT><p>fine</p>"), "ok\nfine");
        let p = parse("<p>a<!-- <b>x</b> > y -->b</p><svg><text>logo</text></svg><p>c</p>");
        assert_eq!(plain("<p>a<!-- <b>x</b> > y -->b</p><svg><text>logo</text></svg><p>c</p>"), "ab\nc");
        assert_eq!(p.blocks.len(), 2);
    }
}
