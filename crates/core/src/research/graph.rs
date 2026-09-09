//! Research v2 step 0: what the knowledge base already knows, and how
//! much a round added to it.

use super::digest::{normalize_for_match, Digest};
use crate::kms::KmsRef;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct KnownNote {
    pub slug: String,
    pub title: String,
    pub summary: String,
    /// `kind:` frontmatter (`entity` / `concept` / `claim` / `moc`), empty when absent.
    pub kind: String,
    /// `updated:` frontmatter (`YYYY-MM-DD`) when present.
    pub updated: Option<String>,
}

/// Every page in the KMS as `(slug, title, first prose line)`. Reads
/// files, never the LLM. Skips `_summary` and other underscore pages.
pub fn load_known(kref: &KmsRef) -> Vec<KnownNote> {
    let dir = kref.root.join("pages");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.starts_with('_') || stem.starts_with('.') {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (fm, body) = crate::kms::parse_frontmatter(&raw);
        let title = fm
            .get("title")
            .map(|t| t.trim_matches('"').to_string())
            .unwrap_or_else(|| stem.to_string());
        out.push(KnownNote {
            slug: stem.to_string(),
            title,
            summary: first_prose_line(&body),
            kind: fm.get("kind").cloned().unwrap_or_default(),
            updated: fm.get("updated").cloned().filter(|u| !u.is_empty()),
        });
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    out
}

/// First line that is not a heading, the injected `Description:` line,
/// a rule, or blank. Clamped to 200 chars.
pub fn first_prose_line(body: &str) -> String {
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty()
            || t.starts_with('#')
            || t.starts_with("Description:")
            || t.starts_with("---")
            || t.starts_with("```")
        {
            continue;
        }
        let mut s: String = t.chars().take(200).collect();
        if t.chars().count() > 200 {
            s.push('…');
        }
        return s;
    }
    String::new()
}

/// Deterministic stop signal: the share of a round's entities + claims
/// that the run had not seen before.
#[derive(Debug, Default)]
pub struct Novelty {
    entities: HashSet<String>,
    claims: HashSet<String>,
}

impl Novelty {
    /// Fold a round's digests in; returns `(new_items, total_items_in_round, ratio)`.
    pub fn absorb(&mut self, round: &[Digest]) -> (u32, u32, f32) {
        let mut new = 0u32;
        let mut total = 0u32;
        let mut ent_new = 0u32;
        let mut ent_total = 0u32;
        for d in round {
            for e in &d.entities {
                total += 1;
                ent_total += 1;
                if self.entities.insert(e.slug.clone()) {
                    new += 1;
                    ent_new += 1;
                }
            }
            for c in &d.claims {
                total += 1;
                let key: String = normalize_for_match(&c.text).chars().take(120).collect();
                if self.claims.insert(key) {
                    new += 1;
                }
            }
        }
        // Stop signal uses entities only: they are LLM-normalised slugs
        // that repeat across sources, while claim text is reworded by
        // every source and would keep novelty near 100% forever.
        let ratio = if ent_total == 0 {
            if total == 0 {
                0.0
            } else {
                new as f32 / total as f32
            }
        } else {
            ent_new as f32 / ent_total as f32
        };
        (new, total, ratio)
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }
    pub fn claim_count(&self) -> usize {
        self.claims.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::digest::{Claim, Entity};

    fn d(ents: &[&str], claims: &[&str]) -> Digest {
        Digest {
            url: "u".into(),
            title: "t".into(),
            fetched: "d".into(),
            source: 1,
            entities: ents
                .iter()
                .map(|e| Entity {
                    name: e.to_string(),
                    slug: e.to_string(),
                    kind: String::new(),
                })
                .collect(),
            claims: claims
                .iter()
                .enumerate()
                .map(|(i, c)| Claim {
                    id: format!("s1c{i}"),
                    text: c.to_string(),
                    quote: c.to_string(),
                    entities: vec![],
                    confidence: 0.8,
                    source: 1,
                    published: None,
                })
                .collect(),
            links_to_known: vec![],
            dropped_claims: 0,
            model: String::new(),
            published: None,
        }
    }

    #[test]
    fn novelty_drops_as_rounds_repeat() {
        let mut n = Novelty::default();
        let (new, total, r) = n.absorb(&[d(&["a", "b"], &["x", "y"])]);
        assert_eq!((new, total), (4, 4));
        assert!((r - 1.0).abs() < 1e-6);
        let (new, total, r) = n.absorb(&[d(&["a", "c"], &["x", "Y "])]);
        assert_eq!((new, total), (1, 4));
        assert!((r - 0.5).abs() < 1e-6, "entity-based: 1 new of 2 entities");
        assert_eq!(n.entity_count(), 3);
        assert_eq!(n.claim_count(), 2);
    }

    #[test]
    fn first_prose_line_skips_header_boilerplate() {
        let body = "# Title\nDescription: x\n---\n\n## Heading\nThe real abstract. More.\n";
        assert_eq!(first_prose_line(body), "The real abstract. More.");
    }

    #[test]
    fn load_known_reads_pages() {
        let _h = crate::research::test_helpers::scoped_home();
        let kref = crate::kms::create("known-rt", crate::kms::KmsScope::Project).unwrap();
        crate::kms::write_page(
            &kref,
            "overtime-pay",
            "---\ntitle: \"Overtime pay\"\n---\n\nOvertime is 1.5x.\n",
        )
        .unwrap();
        crate::kms::write_page(&kref, "_summary", "---\ntitle: s\n---\n\nx\n").unwrap();
        let k = load_known(&kref);
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].slug, "overtime-pay");
        assert_eq!(k[0].title, "Overtime pay");
        assert_eq!(k[0].summary, "Overtime is 1.5x.");
    }
}
