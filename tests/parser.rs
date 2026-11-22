use rust_hosts::Error;
use rust_hosts::block::{BEGIN, Block, Document, END, Entry, Eol, Group};

const NO_BLOCK: &str = "\
127.0.0.1 localhost
::1 localhost ip6-localhost
# fim do arquivo
";

const BLOCK_AT_TOP: &str = "\
# BEGIN rust-hosts
# Dev
127.0.0.1 dev.example.com
# END rust-hosts
127.0.0.1 localhost
";

const BLOCK_IN_MIDDLE: &str = "\
127.0.0.1 localhost

# BEGIN rust-hosts
# Dev
10.0.0.1 dev.example.com
# END rust-hosts
::1 localhost
";

const BLOCK_AT_END: &str = "\
127.0.0.1 localhost
# BEGIN rust-hosts
# Dev
10.0.0.1 dev.example.com
# END rust-hosts
";

const EMPTY_GROUPS: &str = "\
# BEGIN rust-hosts
# Vazio A
# Vazio B
# END rust-hosts
";

const DISABLED_AND_IPV6: &str = "\
# BEGIN rust-hosts
# Homolog
# 172.25.0.80 hml.example.com
# IPv6
2001:db8::1 example.com
::1 localhost
# END rust-hosts
";

const TABS: &str = "\
# BEGIN rust-hosts
# Dev
127.0.0.1\tdev.example.com  \t  other.example.com\t
# END rust-hosts
";

const NO_FINAL_NEWLINE: &str = "\
# BEGIN rust-hosts
# Dev
127.0.0.1 dev.example.com
# END rust-hosts";

const VALID: [&str; 7] = [
    NO_BLOCK,
    BLOCK_AT_TOP,
    BLOCK_IN_MIDDLE,
    BLOCK_AT_END,
    EMPTY_GROUPS,
    DISABLED_AND_IPV6,
    TABS,
];

const CANONICAL: [&str; 5] = [
    NO_BLOCK,
    BLOCK_AT_TOP,
    BLOCK_IN_MIDDLE,
    BLOCK_AT_END,
    EMPTY_GROUPS,
];

fn parse(text: &str) -> Document {
    Document::parse(text).expect("fixture deveria ser válido")
}

fn parse_err(text: &str) -> Error {
    Document::parse(text).expect_err("fixture deveria falhar")
}

/// Invariante 1: `parse(render(parse(x))) == parse(x)`.
fn assert_round_trip(text: &str) {
    let first = parse(text);
    let rendered = first.render();
    assert_eq!(parse(&rendered), first, "round-trip instável para:\n{text}");
}

/// Invariante 3: bytes fora do bloco idênticos após render.
fn assert_outside_preserved(text: &str) {
    let doc = parse(text);
    let rendered = doc.render();
    assert!(
        rendered.starts_with(doc.before.as_str()),
        "prefixo alterado em:\n{text}"
    );
    if !doc.after.is_empty() {
        assert!(
            rendered.ends_with(doc.after.as_str()),
            "sufixo alterado em:\n{text}"
        );
    }
}

#[test]
fn sem_bloco_devolve_o_arquivo_intacto() {
    let doc = parse(NO_BLOCK);
    assert!(doc.block.is_none());
    assert!(!doc.has_block());
    assert_eq!(doc.before, NO_BLOCK);
    assert_eq!(doc.after, "");
    assert_eq!(doc.render(), NO_BLOCK);
}

#[test]
fn bloco_canonico_renderiza_identico() {
    for fixture in CANONICAL {
        assert_eq!(parse(fixture).render(), fixture, "fixture não canônico");
    }
}

#[test]
fn round_trip_e_bytes_externos() {
    for fixture in VALID.iter().chain([NO_FINAL_NEWLINE].iter()) {
        assert_round_trip(fixture);
        assert_outside_preserved(fixture);
    }
}

#[test]
fn parse_do_bloco_no_topo_no_meio_e_no_fim() {
    let top = parse(BLOCK_AT_TOP);
    assert_eq!(top.before, "");
    assert_eq!(top.after, "127.0.0.1 localhost\n");

    let middle = parse(BLOCK_IN_MIDDLE);
    assert_eq!(middle.before, "127.0.0.1 localhost\n\n");
    assert_eq!(middle.after, "::1 localhost\n");

    let end = parse(BLOCK_AT_END);
    assert_eq!(end.before, "127.0.0.1 localhost\n");
    assert_eq!(end.after, "");
}

#[test]
fn grupos_vazios_persistem() {
    let doc = parse(EMPTY_GROUPS);
    let block = doc.block.as_ref().expect("bloco presente");
    let names: Vec<_> = block.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Vazio A", "Vazio B"]);
    assert!(block.groups.iter().all(|g| g.entries.is_empty()));
    assert_eq!(doc.render(), EMPTY_GROUPS);
}

#[test]
fn entrada_desabilitada_e_ipv6() {
    let doc = parse(DISABLED_AND_IPV6);
    let block = doc.block.as_ref().expect("bloco presente");
    let disabled = &block.groups[0].entries[0];
    assert!(!disabled.enabled);
    assert_eq!(disabled.ip.to_string(), "172.25.0.80");
    assert_eq!(disabled.hostnames, ["hml.example.com"]);

    let ipv6 = &block.groups[1].entries[0];
    assert!(ipv6.enabled);
    assert_eq!(ipv6.ip.to_string(), "2001:db8::1");
    assert!(block.groups[1].entries[1].ip.is_ipv6());
}

#[test]
fn tabs_e_espacos_normalizam_para_um_espaco() {
    let rendered = parse(TABS).render();
    assert_eq!(
        rendered,
        "# BEGIN rust-hosts\n# Dev\n127.0.0.1 dev.example.com other.example.com\n# END rust-hosts\n"
    );
}

#[test]
fn sem_newline_final_ganha_eol_ao_renderizar() {
    let doc = parse(NO_FINAL_NEWLINE);
    assert_eq!(doc.after, "");
    let rendered = doc.render();
    assert!(rendered.ends_with(&format!("{END}\n")));
    assert_round_trip(NO_FINAL_NEWLINE);
}

#[test]
fn bloco_novo_no_fim_sem_newline_final() {
    let doc = Document {
        before: "127.0.0.1 localhost".to_owned(),
        block: Some(Block {
            groups: vec![Group {
                name: "Novo".to_owned(),
                entries: vec![
                    Entry::new(
                        "10.0.0.1".parse().unwrap(),
                        vec!["a.example.com".to_owned()],
                        true,
                    )
                    .unwrap(),
                ],
            }],
        }),
        after: String::new(),
        eol: Eol::Lf,
    };
    let rendered = doc.render();
    assert_eq!(
        rendered,
        "127.0.0.1 localhost\n# BEGIN rust-hosts\n# Novo\n10.0.0.1 a.example.com\n# END rust-hosts\n"
    );
    assert_eq!(parse(&rendered).before, "127.0.0.1 localhost\n");
}

#[test]
fn crlf_e_dominante_e_preservado() {
    let text = BLOCK_IN_MIDDLE.replace('\n', "\r\n");
    let doc = parse(&text);
    assert_eq!(doc.eol, Eol::Crlf);
    assert_eq!(doc.render(), text);
    assert_eq!(
        doc.render().matches("\r\n").count(),
        text.matches('\n').count()
    );
    assert_round_trip(&text);
    assert_eq!(parse(BLOCK_IN_MIDDLE).eol, Eol::Lf);
}

#[test]
fn marcador_com_espacos_no_fim_continua_marcador() {
    let text = format!("{BEGIN}  \n# Dev\n{END}\t\n");
    let doc = parse(&text);
    assert!(doc.has_block());
    assert_eq!(doc.render(), format!("{BEGIN}\n# Dev\n{END}\n"));
}

#[test]
fn erros_de_marcador() {
    for text in [
        "# BEGIN rust-hosts\n# BEGIN rust-hosts\n# END rust-hosts\n",
        "# END rust-hosts\n# BEGIN rust-hosts\n",
        "# BEGIN rust-hosts\n# Dev\n",
        "# END rust-hosts\n",
    ] {
        let error = parse_err(text);
        assert!(matches!(error, Error::MalformedBlock), "{error}");
    }
}

#[test]
fn erro_de_entrada_fora_de_grupo() {
    let text = "# BEGIN rust-hosts\n127.0.0.1 foo.com\n# Dev\n# END rust-hosts\n";
    let error = parse_err(text);
    assert!(
        matches!(error, Error::EntryOutsideGroup(ref line) if line == "127.0.0.1 foo.com"),
        "{error}"
    );
}

#[test]
fn erro_de_ip_invalido() {
    let text = "# BEGIN rust-hosts\n# Dev\n999.0.0.1 foo.com\n# END rust-hosts\n";
    let error = parse_err(text);
    assert!(
        matches!(error, Error::InvalidIp(ref ip) if ip == "999.0.0.1"),
        "{error}"
    );
}

#[test]
fn erro_de_hostname_vazio_ou_com_espaco() {
    let text = "# BEGIN rust-hosts\n# Dev\n127.0.0.1\n# END rust-hosts\n";
    let error = parse_err(text);
    assert!(
        matches!(error, Error::InvalidHostname(ref host) if host.is_empty()),
        "{error}"
    );

    let ip = "127.0.0.1".parse().unwrap();
    let error = Entry::new(ip, vec!["has space".to_owned()], true).expect_err("deveria falhar");
    assert!(
        matches!(error, Error::InvalidHostname(ref host) if host == "has space"),
        "{error}"
    );
    let error = Entry::new(ip, Vec::new(), true).expect_err("deveria falhar");
    assert!(matches!(error, Error::InvalidHostname(ref host) if host.is_empty()));
    let error = Entry::new(ip, vec!["a\u{7}b".to_owned()], false).expect_err("deveria falhar");
    assert!(matches!(error, Error::InvalidHostname(_)), "{error}");
}

#[test]
fn erro_de_grupo_invalido() {
    let empty = "# BEGIN rust-hosts\n#\n# END rust-hosts\n";
    assert!(
        matches!(parse_err(empty), Error::InvalidGroupName(ref name) if name.is_empty()),
        "grupo sem nome deveria falhar"
    );

    let marker = "# BEGIN rust-hosts\n#  BEGIN rust-hosts\n# END rust-hosts\n";
    assert!(
        matches!(parse_err(marker), Error::InvalidGroupName(ref name) if name == "BEGIN rust-hosts"),
        "grupo com nome de marcador deveria falhar"
    );
}

#[test]
fn entry_new_monta_entrada_valida() {
    let ip = "::1".parse().unwrap();
    let entry = Entry::new(ip, vec!["localhost".to_owned()], false).unwrap();
    assert_eq!(entry.ip, ip);
    assert_eq!(entry.hostnames, ["localhost"]);
    assert!(!entry.enabled);
}
