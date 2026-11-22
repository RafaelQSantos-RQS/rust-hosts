//! Bloco gerenciado: modelo, parser e serializador (ADR-0004, ADR-0005).

use std::net::IpAddr;

use crate::Error;

pub const BEGIN: &str = "# BEGIN rust-hosts";
pub const END: &str = "# END rust-hosts";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Eol {
    #[default]
    Lf,
    Crlf,
}

impl Eol {
    fn as_str(self) -> &'static str {
        match self {
            Eol::Lf => "\n",
            Eol::Crlf => "\r\n",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    /// Bytes originais antes do bloco, incluindo os EOLs.
    pub before: String,
    /// `None` quando o arquivo não tem bloco.
    pub block: Option<Block>,
    /// Bytes originais depois do bloco (`""` quando não há bloco).
    pub after: String,
    /// EOL dominante do arquivo, usado ao serializar o bloco.
    pub eol: Eol,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Block {
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub ip: IpAddr,
    pub hostnames: Vec<String>,
    pub enabled: bool,
}

impl Document {
    pub fn parse(input: &str) -> Result<Self, Error> {
        let lines = split_lines(input);
        let eol = dominant_eol(input);

        let mut begin = None;
        let mut end = None;
        for (index, line) in lines.iter().enumerate() {
            let marker = match line.text.trim_end() {
                BEGIN => &mut begin,
                END => &mut end,
                _ => continue,
            };
            if marker.replace(index).is_some() {
                return Err(Error::MalformedBlock);
            }
        }

        let (begin, end) = match (begin, end) {
            (None, None) => {
                return Ok(Document {
                    before: input.to_owned(),
                    block: None,
                    after: String::new(),
                    eol,
                });
            }
            (Some(begin), Some(end)) if begin < end => (begin, end),
            _ => return Err(Error::MalformedBlock),
        };

        let mut block = Block::default();
        for line in &lines[begin + 1..end] {
            parse_block_line(line.text.trim(), &mut block)?;
        }

        Ok(Document {
            before: input[..lines[begin].start].to_owned(),
            block: Some(block),
            after: input[lines[end].next..].to_owned(),
            eol,
        })
    }

    pub fn render(&self) -> String {
        let mut out = self.before.clone();
        if self.block.is_none() {
            return out;
        }

        // Bloco novo criado no fim de um arquivo sem EOL final.
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(self.eol.as_str());
        }

        out.push_str(&self.block_text());
        out.push_str(&self.after);
        out
    }

    /// Só o bloco, sem o resto do arquivo: é o que o Apply compara e envia.
    pub fn block_text(&self) -> String {
        let Some(block) = &self.block else {
            return String::new();
        };

        let eol = self.eol.as_str();
        let mut out = String::new();
        out.push_str(BEGIN);
        out.push_str(eol);
        for group in &block.groups {
            out.push_str("# ");
            out.push_str(&group.name);
            out.push_str(eol);
            for entry in &group.entries {
                if !entry.enabled {
                    out.push_str("# ");
                }
                out.push_str(&entry.ip.to_string());
                for hostname in &entry.hostnames {
                    out.push(' ');
                    out.push_str(hostname);
                }
                out.push_str(eol);
            }
        }
        out.push_str(END);
        out.push_str(eol);
        out
    }

    pub fn has_block(&self) -> bool {
        self.block.is_some()
    }
}

impl Group {
    /// Nome obrigatório e diferente dos marcadores, senão o bloco não reparseia.
    pub fn new(name: impl Into<String>) -> Result<Self, Error> {
        let name = name.into();
        let trimmed = name.trim();
        if trimmed.is_empty() || is_marker_name(trimmed) {
            return Err(Error::InvalidGroupName(trimmed.to_owned()));
        }
        Ok(Self {
            name: trimmed.to_owned(),
            entries: Vec::new(),
        })
    }
}

impl Entry {
    /// Valida hostnames (não vazio, sem whitespace nem caracteres de controle) e monta a entrada.
    pub fn new(ip: IpAddr, hostnames: Vec<String>, enabled: bool) -> Result<Self, Error> {
        if hostnames.is_empty() {
            return Err(Error::InvalidHostname(String::new()));
        }
        for hostname in &hostnames {
            if hostname.is_empty()
                || hostname
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control())
            {
                return Err(Error::InvalidHostname(hostname.clone()));
            }
        }
        Ok(Self {
            ip,
            hostnames,
            enabled,
        })
    }
}

pub(crate) struct Line<'a> {
    pub(crate) start: usize,
    /// Linha sem o terminador (`\n` ou `\r\n`).
    pub(crate) text: &'a str,
    /// Índice logo após o terminador.
    pub(crate) next: usize,
}

pub(crate) fn split_lines(input: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in input.bytes().enumerate() {
        if byte != b'\n' {
            continue;
        }
        let mut text_end = index;
        if text_end > start && input.as_bytes()[text_end - 1] == b'\r' {
            text_end -= 1;
        }
        lines.push(Line {
            start,
            text: &input[start..text_end],
            next: index + 1,
        });
        start = index + 1;
    }
    if start < input.len() {
        lines.push(Line {
            start,
            text: &input[start..],
            next: input.len(),
        });
    }
    lines
}

fn dominant_eol(input: &str) -> Eol {
    let lf = input.matches('\n').count();
    let crlf = input.matches("\r\n").count();
    if crlf * 2 > lf { Eol::Crlf } else { Eol::Lf }
}

fn is_marker_name(name: &str) -> bool {
    BEGIN.strip_prefix("# ") == Some(name) || END.strip_prefix("# ") == Some(name)
}

fn parse_block_line(line: &str, block: &mut Block) -> Result<(), Error> {
    if line.is_empty() {
        return Ok(());
    }

    if let Some(rest) = line.strip_prefix('#') {
        let rest = rest.trim();
        let mut parts = rest.split_whitespace();
        // `# <ip> <hostnames>` é entrada desabilitada; qualquer outro `#...` é cabeçalho.
        if let Some(first) = parts.next()
            && let Ok(ip) = first.parse::<IpAddr>()
            && parts.clone().next().is_some()
        {
            return push_entry(block, ip, parts.map(str::to_owned).collect(), false, line);
        }
        if rest.is_empty() || is_marker_name(rest) {
            return Err(Error::InvalidGroupName(rest.to_owned()));
        }
        block.groups.push(Group::new(rest)?);
        return Ok(());
    }

    let mut parts = line.split_whitespace();
    let Some(first) = parts.next() else {
        return Ok(());
    };
    let ip = first
        .parse::<IpAddr>()
        .map_err(|_| Error::InvalidIp(first.to_owned()))?;
    push_entry(block, ip, parts.map(str::to_owned).collect(), true, line)
}

fn push_entry(
    block: &mut Block,
    ip: IpAddr,
    hostnames: Vec<String>,
    enabled: bool,
    line: &str,
) -> Result<(), Error> {
    let Some(group) = block.groups.last_mut() else {
        return Err(Error::EntryOutsideGroup(line.to_owned()));
    };
    group.entries.push(Entry::new(ip, hostnames, enabled)?);
    Ok(())
}
