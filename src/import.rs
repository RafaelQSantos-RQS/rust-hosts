//! Importação: mover entradas de fora do bloco para dentro dele (design.md, "Importação").

use std::net::IpAddr;

use crate::block::{Document, Entry, Group, split_lines};

/// Onde a linha candidata está: o bloco divide o arquivo em duas regiões.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Before,
    After,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub region: Region,
    /// Faixa da linha no texto original, incluindo o terminador.
    pub start: usize,
    pub end: usize,
    /// Último comentário acima da entrada; `None` vira o grupo "Ungrouped".
    pub group: Option<String>,
    pub entry: Entry,
}

impl Candidate {
    /// Hostnames de sistema: o Preview começa com eles desmarcados.
    pub fn is_system(&self) -> bool {
        self.entry
            .hostnames
            .iter()
            .any(|hostname| hostname == "localhost" || hostname.starts_with("ip6-"))
    }
}

pub const UNGROUPED: &str = "Ungrouped";

pub fn scan(document: &Document) -> Vec<Candidate> {
    let mut candidates = scan_region(Region::Before, &document.before);
    candidates.extend(scan_region(Region::After, &document.after));
    candidates
}

/// Devolve o arquivo novo: as linhas escolhidas saem do lugar original e entram no bloco.
pub fn import(document: &Document, selected: &[Candidate]) -> String {
    let mut next = document.clone();
    next.before = remove(&document.before, selected, Region::Before);
    next.after = remove(&document.after, selected, Region::After);

    let mut block = document.block.clone().unwrap_or_default();
    for candidate in selected {
        // Nome validado: um comentário não pode virar um cabeçalho que não reparseia.
        let name = candidate
            .group
            .as_deref()
            .and_then(|name| Group::new(name).ok())
            .map_or_else(|| UNGROUPED.to_owned(), |group| group.name);
        let group = match block.groups.iter().position(|group| group.name == name) {
            Some(index) => index,
            None => {
                block.groups.push(Group {
                    name,
                    entries: Vec::new(),
                });
                block.groups.len() - 1
            }
        };
        block.groups[group].entries.push(candidate.entry.clone());
    }

    next.block = Some(block);
    next.render()
}

fn scan_region(region: Region, text: &str) -> Vec<Candidate> {
    let mut group: Option<String> = None;
    let mut candidates = Vec::new();

    for line in split_lines(text) {
        let trimmed = line.text.trim();

        if let Some(comment) = trimmed.strip_prefix('#') {
            let comment = comment.trim();
            // `# <ip> <hostnames>` é entrada desabilitada (ADR-0005); o resto é cabeçalho.
            match parse_entry(comment) {
                Some((ip, hostnames)) => {
                    if let Ok(entry) = Entry::new(ip, hostnames, false) {
                        candidates.push(Candidate {
                            region,
                            start: line.start,
                            end: line.next,
                            group: group.clone(),
                            entry,
                        });
                    }
                }
                None if comment.is_empty() => group = None,
                None => group = Some(comment.to_owned()),
            }
        } else if let Some((ip, hostnames)) = parse_entry(trimmed)
            && let Ok(entry) = Entry::new(ip, hostnames, true)
        {
            candidates.push(Candidate {
                region,
                start: line.start,
                end: line.next,
                group: group.clone(),
                entry,
            });
        }
    }

    candidates
}

/// Linhas com comentário depois dos hostnames não são importáveis; ficam onde estão.
fn parse_entry(text: &str) -> Option<(IpAddr, Vec<String>)> {
    let mut parts = text.split_whitespace();
    let ip = parts.next()?.parse().ok()?;
    let rest: Vec<&str> = parts.collect();
    if rest.is_empty() || rest.iter().any(|token| token.starts_with('#')) {
        return None;
    }
    Some((ip, rest.into_iter().map(str::to_owned).collect()))
}

fn remove(text: &str, selected: &[Candidate], region: Region) -> String {
    let mut ranges: Vec<(usize, usize)> = selected
        .iter()
        .filter(|candidate| candidate.region == region)
        .map(|candidate| (candidate.start, candidate.end))
        .collect();
    ranges.sort_unstable();

    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for (start, end) in ranges {
        if start < cursor {
            continue;
        }
        out.push_str(&text[cursor..start]);
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNMANAGED: &str = "\
127.0.0.1 localhost
# Dev
10.1.1.1 dev.test
# 10.1.1.2 penpot.test
# Prod
# 172.25.0.80 hml.test
";

    fn document(text: &str) -> Document {
        Document::parse(text).unwrap()
    }

    #[test]
    fn scan_groups_entries_by_the_comment_above_them() {
        let candidates = scan(&document(UNMANAGED));

        let names: Vec<_> = candidates
            .iter()
            .map(|candidate| (candidate.group.as_deref(), candidate.entry.enabled))
            .collect();

        assert_eq!(
            names,
            vec![
                (None, true),
                (Some("Dev"), true),
                (Some("Dev"), false),
                (Some("Prod"), false),
            ]
        );
        assert!(candidates[0].is_system());
        assert!(!candidates[1].is_system());
    }

    #[test]
    fn import_moves_the_lines_into_the_block() {
        let document = document(UNMANAGED);
        let candidates = scan(&document);
        let selected = [candidates[1].clone(), candidates[3].clone()];

        let result = import(&document, &selected);

        assert_eq!(
            result,
            "\
127.0.0.1 localhost
# Dev
# 10.1.1.2 penpot.test
# Prod
# BEGIN rust-hosts
# Dev
10.1.1.1 dev.test
# Prod
# 172.25.0.80 hml.test
# END rust-hosts
"
        );
        assert!(Document::parse(&result).is_ok());
    }

    #[test]
    fn import_creates_the_ungrouped_group() {
        let document = document("10.0.0.5 api.test\n");
        let candidates = scan(&document);

        let result = import(&document, &candidates);

        assert_eq!(
            result,
            "# BEGIN rust-hosts\n# Ungrouped\n10.0.0.5 api.test\n# END rust-hosts\n"
        );
    }

    #[test]
    fn import_merges_into_an_existing_group_and_keeps_the_rest() {
        let text = "\
10.0.0.9 keep.test
# Dev
10.0.0.2 dev.test
# BEGIN rust-hosts
# Dev
10.0.0.1 local.dev
# END rust-hosts
";
        let document = document(text);
        let selected: Vec<_> = scan(&document)
            .into_iter()
            .filter(|candidate| candidate.group.is_some())
            .collect();

        let result = import(&document, &selected);

        assert_eq!(
            result,
            "\
10.0.0.9 keep.test
# Dev
# BEGIN rust-hosts
# Dev
10.0.0.1 local.dev
10.0.0.2 dev.test
# END rust-hosts
"
        );
    }

    #[test]
    fn scan_skips_lines_with_a_trailing_comment() {
        let candidates = scan(&document("10.0.0.7 foo.test # note\n"));

        assert!(candidates.is_empty());
    }

    #[test]
    fn a_bare_hash_is_not_a_group_header() {
        let document = document("#\n10.0.0.9 orphan.test\n");
        let candidates = scan(&document);
        assert_eq!(candidates[0].group, None);

        let result = import(&document, &candidates);

        assert!(Document::parse(&result).is_ok());
        assert!(result.contains("# Ungrouped\n10.0.0.9 orphan.test\n"));
    }
}
