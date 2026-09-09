//! Which pages a round actually reads.
//!
//! Search engines rank for engagement, not for evidence: a query about
//! a vendor's newest model returns the vendor's own release notes and
//! five SEO listicles that paraphrase it, and the listicles often rank
//! higher. A run that fetches the top N per query therefore spends most
//! of its budget on restatements — the 2026-09-07 "Chinese AI
//! companies" run read `techjacksolutions.com`, `mysummit.school` and
//! `geotoolbox.ai` while `deepseek.com` never appeared.
//!
//! Three deterministic passes fix that without an LLM call:
//!
//! 1. [`canonical_url`] — one page is one source. Tracking parameters,
//!    fragments, `www.`, `http` vs `https` and a trailing slash used to
//!    produce two sources, two digests and two citation numbers for the
//!    same document.
//! 2. [`tier`] — primary sources (the domain the query is about,
//!    official docs, gov/edu, arXiv, Wikipedia, major press) sort ahead
//!    of everything else. Nothing is ever excluded; ordering only
//!    decides who gets read first when a round can afford N pages.
//! 3. [`rank_candidates`] — caps how many pages one domain contributes
//!    to a round, so a single site cannot supply half the evidence.

use super::pipeline::SearchHit;
use std::collections::HashMap;

/// Pages one domain may contribute to a single round.
pub const MAX_PER_DOMAIN_PER_ROUND: usize = 2;

/// Two-level public suffixes common enough to matter for
/// [`registrable_domain`]. Not a full public-suffix list: getting
/// `co.uk` and `ac.th` right covers the cases where a naive
/// "last two labels" would collapse unrelated sites into one domain.
const TWO_LEVEL_SUFFIXES: &[&str] = &[
    "co.uk", "org.uk", "ac.uk", "gov.uk", "co.jp", "or.jp", "ne.jp", "ac.jp", "go.jp", "co.th",
    "ac.th", "go.th", "or.th", "in.th", "com.au", "net.au", "org.au", "edu.au", "gov.au", "com.cn",
    "net.cn", "org.cn", "edu.cn", "gov.cn", "com.br", "com.tw", "com.hk", "com.sg", "com.my",
    "co.kr", "or.kr", "co.in", "co.nz", "co.za",
];

/// Reference works and news organisations that report rather than
/// aggregate. Deliberately short: this is a tie-break, not a
/// whitelist, and every domain not listed still gets read.
const REFERENCE_DOMAINS: &[&str] = &[
    "wikipedia.org",
    "reuters.com",
    "apnews.com",
    "bloomberg.com",
    "ft.com",
    "wsj.com",
    "nytimes.com",
    "economist.com",
    "bbc.com",
    "bbc.co.uk",
    "nature.com",
    "science.org",
    "arxiv.org",
    "acm.org",
    "ieee.org",
    "nikkei.com",
    "scmp.com",
    "theverge.com",
    "arstechnica.com",
    "techcrunch.com",
    "venturebeat.com",
    "theinformation.com",
    "semianalysis.com",
    "github.com",
    "huggingface.co",
    "bangkokpost.com",
    "thairath.co.th",
    "prachachat.net",
];

/// How much a source is worth reading first. Lower sorts earlier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// The subject's own site, official documentation, a government or
    /// academic domain — the thing itself rather than a report of it.
    Primary = 0,
    /// Reference works and news organisations.
    Reference = 1,
    /// Everything else. Read after the above, never dropped.
    Other = 2,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Primary => "primary",
            Tier::Reference => "reference",
            Tier::Other => "other",
        }
    }
}

/// Host without `www.`, lowercased. `None` for a URL with no host.
pub fn host_of(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed
        .host_str()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
}

/// The domain a site is registered under: `blog.deepseek.com` and
/// `deepseek.com` are one publisher, `a.github.io` and `b.github.io`
/// are not (github.io is not in the two-level list, so both collapse
/// to `github.io` — acceptable, they are the same host anyway).
pub fn registrable_domain(url: &str) -> Option<String> {
    let host = host_of(url)?;
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() <= 2 {
        return Some(host);
    }
    let last_two = labels[labels.len() - 2..].join(".");
    let take = if TWO_LEVEL_SUFFIXES.contains(&last_two.as_str()) {
        3
    } else {
        2
    };
    if labels.len() <= take {
        return Some(host);
    }
    Some(labels[labels.len() - take..].join("."))
}

/// Strip what does not identify the document: tracking parameters, the
/// fragment, `www.`, a trailing slash, and the scheme's http/https
/// split. Two URLs with the same canonical form are the same source.
///
/// Not a normaliser for display — the original URL is what gets cited.
/// This is only the dedupe key.
pub fn canonical_url(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.trim().trim_end_matches('/').to_ascii_lowercase();
    };
    parsed.set_fragment(None);
    let keep: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| !is_tracking_param(k))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if keep.is_empty() {
        parsed.set_query(None);
    } else {
        let mut sorted = keep;
        sorted.sort();
        let q = sorted
            .iter()
            .map(|(k, v)| {
                if v.is_empty() {
                    k.clone()
                } else {
                    format!("{k}={v}")
                }
            })
            .collect::<Vec<_>>()
            .join("&");
        parsed.set_query(Some(&q));
    }
    let host = parsed
        .host_str()
        .map(|h| {
            let h = h.trim_end_matches('.').to_ascii_lowercase();
            h.strip_prefix("www.").unwrap_or(&h).to_string()
        })
        .unwrap_or_default();
    let path = parsed.path().trim_end_matches('/');
    let query = parsed.query().map(|q| format!("?{q}")).unwrap_or_default();
    if host.is_empty() {
        return url.trim().trim_end_matches('/').to_ascii_lowercase();
    }
    format!("{host}{path}{query}")
}

fn is_tracking_param(k: &str) -> bool {
    let k = k.to_ascii_lowercase();
    k.starts_with("utm_")
        || matches!(
            k.as_str(),
            "fbclid"
                | "gclid"
                | "gbraid"
                | "wbraid"
                | "msclkid"
                | "mc_cid"
                | "mc_eid"
                | "igshid"
                | "ref"
                | "ref_src"
                | "source"
                | "spm"
                | "_hsenc"
                | "_hsmi"
                | "yclid"
                | "twclid"
        )
}

/// Alphanumeric runs of a string, lowercased — the shape a domain
/// label and a query word have in common (`DeepSeek` ↔ `deepseek`).
fn word_keys(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

/// Rank a source. `subject_words` are the query's words plus the
/// entity names the run has already found — a domain that spells one
/// of them is the subject's own site, which is the one page a
/// researcher would always read first.
pub fn tier(url: &str, subject_words: &[String]) -> Tier {
    let Some(domain) = registrable_domain(url) else {
        return Tier::Other;
    };
    if domain.ends_with(".gov")
        || domain.ends_with(".edu")
        || domain.ends_with(".mil")
        || domain.ends_with(".int")
        || domain.contains(".gov.")
        || domain.contains(".go.")
        || domain.contains(".edu.")
        || domain.contains(".ac.")
    {
        return Tier::Primary;
    }
    // The name in front of the public suffix, e.g. `deepseek` in
    // `deepseek.com`. Compared whole: `ai` inside `openai` must not
    // make every `*.ai` domain primary.
    let name = domain.split('.').next().unwrap_or_default();
    if name.chars().count() >= 3 && subject_words.iter().any(|w| w == name) {
        return Tier::Primary;
    }
    if REFERENCE_DOMAINS.contains(&domain.as_str()) {
        return Tier::Reference;
    }
    Tier::Other
}

/// Words the tiering compares domains against: the query plus every
/// entity name seen so far, split into alphanumeric runs.
pub fn subject_words(query: &str, entity_names: &[String]) -> Vec<String> {
    let mut out = word_keys(query);
    for n in entity_names {
        out.extend(word_keys(n));
    }
    out.sort();
    out.dedup();
    out
}

/// Order this round's candidate hits and cap each domain's share.
///
/// Stable within a tier, so a search engine's own ranking still decides
/// between two equally-tiered pages. Everything above the per-domain
/// cap is kept, but moved behind the diversified head — a round that
/// can afford more pages than there are distinct domains still reads
/// them rather than starving.
pub fn rank_candidates(hits: Vec<SearchHit>, subject_words: &[String]) -> Vec<SearchHit> {
    let mut indexed: Vec<(usize, SearchHit)> = hits.into_iter().enumerate().collect();
    indexed.sort_by(|(ia, a), (ib, b)| {
        tier(&a.url, subject_words)
            .cmp(&tier(&b.url, subject_words))
            .then(ia.cmp(ib))
    });
    let mut per_domain: HashMap<String, usize> = HashMap::new();
    let (mut head, mut tail) = (Vec::new(), Vec::new());
    for (_, h) in indexed {
        let domain = registrable_domain(&h.url).unwrap_or_default();
        let n = per_domain.entry(domain).or_insert(0);
        if *n < MAX_PER_DOMAIN_PER_ROUND {
            *n += 1;
            head.push(h);
        } else {
            tail.push(h);
        }
    }
    head.extend(tail);
    head
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(url: &str) -> SearchHit {
        SearchHit {
            title: url.into(),
            url: url.into(),
            snippet: String::new(),
        }
    }

    #[test]
    fn canonical_url_collapses_the_same_document() {
        let forms = [
            "https://www.example.com/a/b/",
            "http://example.com/a/b",
            "https://example.com/a/b?utm_source=x&utm_medium=y",
            "https://example.com/a/b#section-3",
            "https://EXAMPLE.com/a/b/?fbclid=123",
        ];
        let first = canonical_url(forms[0]);
        for f in &forms {
            assert_eq!(canonical_url(f), first, "{f}");
        }
        assert_eq!(first, "example.com/a/b");
        // Real query parameters identify a different page and stay.
        assert_ne!(canonical_url("https://example.com/p?id=2"), first);
        assert_eq!(
            canonical_url("https://example.com/p?b=2&a=1"),
            canonical_url("https://example.com/p?a=1&b=2"),
            "parameter order is not identity"
        );
        // Unparseable input still yields a stable key.
        assert_eq!(canonical_url("not a url/"), "not a url");
    }

    #[test]
    fn registrable_domain_handles_two_level_suffixes() {
        assert_eq!(
            registrable_domain("https://blog.deepseek.com/x").as_deref(),
            Some("deepseek.com")
        );
        assert_eq!(
            registrable_domain("https://www.bbc.co.uk/news").as_deref(),
            Some("bbc.co.uk")
        );
        assert_eq!(
            registrable_domain("https://news.thairath.co.th/a").as_deref(),
            Some("thairath.co.th")
        );
        assert_eq!(registrable_domain("mailto:x@y.z"), None);
    }

    #[test]
    fn tiers_put_the_subject_and_references_first() {
        let words = subject_words("Chinese AI companies DeepSeek", &["Alibaba Qwen".into()]);
        assert_eq!(
            tier("https://api-docs.deepseek.com/news", &words),
            Tier::Primary
        );
        assert_eq!(tier("https://www.alibaba.com/x", &words), Tier::Primary);
        assert_eq!(tier("https://data.go.th/dataset", &words), Tier::Primary);
        assert_eq!(
            tier("https://en.wikipedia.org/wiki/Qwen", &words),
            Tier::Reference
        );
        assert_eq!(
            tier("https://techjacksolutions.com/ai-tools", &words),
            Tier::Other
        );
        // A short word inside a longer domain name must not promote it.
        let ai = subject_words("ai", &[]);
        assert_eq!(tier("https://openai.com/x", &ai), Tier::Other);
    }

    #[test]
    fn ranking_puts_primary_first_and_caps_one_domain() {
        let words = subject_words("DeepSeek", &[]);
        let hits = vec![
            hit("https://seo.example.com/1"),
            hit("https://seo.example.com/2"),
            hit("https://seo.example.com/3"),
            hit("https://en.wikipedia.org/wiki/DeepSeek"),
            hit("https://deepseek.com/release"),
        ];
        let out = rank_candidates(hits, &words);
        let urls: Vec<&str> = out.iter().map(|h| h.url.as_str()).collect();
        assert_eq!(urls[0], "https://deepseek.com/release", "{urls:?}");
        assert_eq!(
            urls[1], "https://en.wikipedia.org/wiki/DeepSeek",
            "{urls:?}"
        );
        assert_eq!(urls[2], "https://seo.example.com/1", "{urls:?}");
        assert_eq!(urls[3], "https://seo.example.com/2", "{urls:?}");
        assert_eq!(
            urls[4], "https://seo.example.com/3",
            "the over-cap page is deferred, not dropped: {urls:?}"
        );
        assert_eq!(out.len(), 5);
    }
}
