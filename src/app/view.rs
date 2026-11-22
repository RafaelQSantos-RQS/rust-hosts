//! Layout da janela.

use std::collections::HashSet;

use iced::font::Weight;
use iced::widget::{
    Space, button, checkbox, column, container, pick_list, row, rule, scrollable, text, text_input,
    toggler,
};
use iced::{Alignment, Border, Color, Element, Font, Length, Theme};

use super::{
    Confirm, EntryForm, ImportForm, Message, Modal, Pending, RenameForm, RestoreForm, State, Status,
};
use crate::apply::{self, HelperState};
use crate::backup::Backup;
use crate::block::{Entry, Group};
use crate::import;

/// Timestamp curto dos backups do Restore.
const STAMP: &[time::format_description::FormatItem<'static>] =
    time::macros::format_description!("[year]-[month]-[day] [hour]:[minute] UTC");

pub fn view(state: &State) -> Element<'_, Message> {
    // O modal também cobre a tela de erro: Restore é o caminho de recuperação.
    if !matches!(state.modal(), Modal::None) {
        return view_modal(state.modal());
    }

    if let Some(error) = state.load_error() {
        return view_error(state, error);
    }

    column![view_header(state), view_body(state), view_footer(state)]
        .height(Length::Fill)
        .into()
}

fn view_header(state: &State) -> Element<'_, Message> {
    let search = text_input("Search IP, hostname or group", state.search())
        .on_input_maybe((!state.is_busy()).then_some(Message::SearchChanged))
        .padding(8)
        .width(Length::Fill);

    let has_groups = state
        .document()
        .block
        .as_ref()
        .is_some_and(|block| !block.groups.is_empty());

    container(
        row![
            search,
            // Com o bloco vazio o botão primário do corpo já cria o primeiro grupo.
            quiet_action("+ Group", when(state, has_groups, Message::NewGroup)),
            quiet_action("Import", when(state, true, Message::ImportOpen)),
            quiet_action("Restore", when(state, true, Message::RestoreOpen)),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding(12)
    .width(Length::Fill)
    .into()
}

fn view_body(state: &State) -> Element<'_, Message> {
    let Some(block) = state.document().block.as_ref() else {
        return view_no_block();
    };

    let mut groups = column![].spacing(12);
    if block.groups.is_empty() {
        groups = groups.push(
            column![
                text("The managed block is empty. Add a group or import entries from this file.")
                    .style(text::secondary),
                row![
                    primary_action("New group", when(state, true, Message::NewGroup)),
                    action("Import", when(state, true, Message::ImportOpen)),
                ]
                .spacing(8),
            ]
            .spacing(8),
        );
    }
    for (index, group) in block.groups.iter().enumerate() {
        if state.is_visible(group) {
            groups = groups.push(view_group(state, index, group));
        }
    }

    // Busca sem resultado não pode parecer lista vazia: diz o que houve e como sair.
    let search = state.search().trim();
    let visible = block
        .groups
        .iter()
        .filter(|group| state.is_visible(group))
        .count();
    if !search.is_empty() && visible == 0 && !block.groups.is_empty() {
        groups = groups.push(
            column![
                text(format!("No entries match “{search}”.")).style(text::secondary),
                action("Clear search", Some(Message::SearchChanged(String::new())),),
            ]
            .spacing(8),
        );
    }

    let mut content = column![groups].padding(12).spacing(20);
    if let Some(unmanaged) = view_unmanaged(state) {
        content = content.push(unmanaged);
    }

    container(scrollable(content).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_group<'a>(state: &'a State, index: usize, group: &'a Group) -> Element<'a, Message> {
    let collapsed = state.is_collapsed(group);
    let all_enabled = group.entries.iter().all(|entry| entry.enabled);
    let total = group.entries.len();
    let shown = state.visible_entries(group).len();
    // Com busca ativa o grupo pode mostrar só parte das entradas; não esconde isso.
    let count_label = if shown < total {
        format!("{shown} of {total} entries")
    } else if total == 1 {
        "1 entry".to_owned()
    } else {
        format!("{total} entries")
    };

    // Nome + seta são um alvo só: colapsar não depende de acertar uma seta de 14px.
    let title = button(
        row![
            // A seta herda a cor de ação do botão; o nome continua texto normal.
            text(if collapsed { "▸" } else { "▾" }).size(14),
            text(group.name.as_str())
                .font(bold())
                .size(15)
                .style(text::base),
            text(count_label).size(12).style(text::secondary),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .style(quiet_style)
    .padding([6, 4])
    .on_press(Message::ToggleGroupCollapsed(index));

    let header = row![
        toggler(all_enabled).on_toggle_maybe(on_toggle(
            state,
            !group.entries.is_empty(),
            Message::ToggleGroup(index),
        )),
        title,
        Space::new().width(Length::Fill),
        quiet_action("+ Entry", when(state, true, Message::AddEntry(index))),
        quiet_action("Rename", when(state, true, Message::RenameGroup(index))),
        quiet_action(
            "Delete",
            when(state, group.entries.is_empty(), Message::DeleteGroup(index)),
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut body = column![header].spacing(4);
    if !collapsed {
        for (entry_index, entry) in state.visible_entries(group) {
            body = body.push(view_entry(state, index, entry_index, entry));
        }
    }

    container(body)
        .padding(8)
        .width(Length::Fill)
        .style(container::rounded_box)
        .into()
}

fn view_entry<'a>(
    state: &'a State,
    group: usize,
    index: usize,
    entry: &'a Entry,
) -> Element<'a, Message> {
    let faded = dimmed(entry.enabled);

    row![
        toggler(entry.enabled).on_toggle_maybe(on_toggle(
            state,
            true,
            Message::ToggleEntry(group, index),
        )),
        text(entry.ip.to_string())
            .font(Font::MONOSPACE)
            .size(13)
            .style(faded),
        text(entry.hostnames.join(" "))
            .size(13)
            .style(faded)
            .width(Length::Fill),
        quiet_action("Edit", when(state, true, Message::EditEntry(group, index))),
        quiet_action(
            "Delete",
            when(state, true, Message::DeleteEntry(group, index)),
        ),
    ]
    .padding([4, 0])
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn view_unmanaged(state: &State) -> Option<Element<'_, Message>> {
    let candidates = import::scan(state.document());
    if candidates.is_empty() {
        return None;
    }

    let arrow = if state.unmanaged_open() { "▾" } else { "▸" };
    let label = format!("Unmanaged entries ({})", candidates.len());

    let header = button(
        row![
            text(arrow).size(14),
            text(label).size(14).font(bold()).style(text::base),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .style(quiet_style)
    .padding([6, 4])
    .on_press(Message::ToggleUnmanaged);

    let mut section = column![header].spacing(4);
    if state.unmanaged_open() {
        section = section.push(
            text("Lines outside the managed block. Import moves them in; this list is read-only.")
                .size(12)
                .style(text::secondary),
        );
        for candidate in &candidates {
            section = section.push(
                row![
                    text(candidate.entry.ip.to_string())
                        .font(Font::MONOSPACE)
                        .size(13)
                        .style(text::secondary),
                    text(candidate.entry.hostnames.join(" "))
                        .size(13)
                        .style(text::secondary)
                        .width(Length::Fill),
                ]
                .padding([2, 0])
                .spacing(8),
            );
        }
    }

    // Mesmo cartão dos grupos: a seção é um bloco, não uma linha solta.
    Some(
        container(section)
            .padding(8)
            .width(Length::Fill)
            .style(container::rounded_box)
            .into(),
    )
}

fn view_footer(state: &State) -> Element<'_, Message> {
    let content: Element<'_, Message> = if state.is_busy() {
        // Doherty: durante a espera o app diz onde a ação está, sem prometer o que não sabe.
        column![
            text("Waiting for authentication…").size(14),
            text("A system dialog asks for your password. Nothing is written until you approve.")
                .size(12)
                .style(text::secondary),
        ]
        .spacing(2)
        .into()
    } else {
        let mut bar = row![state_label(state)]
            .spacing(16)
            .align_y(Alignment::Center);
        if let Some(status) = state.status() {
            bar = bar.push(view_status(state, status));
        }
        bar.push(Space::new().width(Length::Fill))
            .push(quiet_action("Reload", when(state, true, Message::Reload)))
            .push(primary_action(
                "Apply",
                when(
                    state,
                    state.document().has_block() && state.helper_ready() && state.is_dirty(),
                    Message::ApplyPressed,
                ),
            ))
            .into()
    };

    let mut footer = column![];
    if let Some(banner) = view_helper(state) {
        footer = footer.push(banner);
    }

    footer
        .push(rule::horizontal(1))
        .push(container(content).padding(12).width(Length::Fill))
        .into()
}

/// Faixa permanente enquanto o helper instalado não serve: o Apply fica desligado.
fn view_helper(state: &State) -> Option<Element<'_, Message>> {
    let headline = match state.helper() {
        HelperState::Ready => return None,
        HelperState::Missing => {
            "The privileged helper is not installed, so Apply is disabled. Install it from here:"
        }
        HelperState::Outdated => {
            "The installed helper does not match this version of the app, so Apply is disabled. Install it again from here:"
        }
    };

    let mut content = column![text(headline).size(12).style(text::danger)].spacing(6);

    match state.helper_source() {
        Some(source) => {
            content = content.push(
                text(apply::install_command(&source))
                    .size(11)
                    .font(Font::MONOSPACE)
                    .style(text::secondary),
            );
            content = content.push(
                row![
                    quiet_action("Copy command", when(state, true, Message::CopyCommand)),
                    quiet_action("Recheck", when(state, true, Message::CheckHelper)),
                    primary_action("Install helper", when(state, true, Message::InstallHelper)),
                ]
                .spacing(8),
            );
        }
        None => {
            content = content.push(
                text("Build it with `cargo build --release` so that rust-hosts-helper sits next to the app.")
                    .size(11)
                    .style(text::secondary),
            );
            content = content.push(row![action(
                "Recheck",
                when(state, true, Message::CheckHelper)
            )]);
        }
    }

    Some(
        container(content)
            .padding([8, 12])
            .width(Length::Fill)
            .style(container::rounded_box)
            .into(),
    )
}

fn state_label(state: &State) -> Element<'_, Message> {
    if state.is_dirty() {
        text("Unsaved changes")
            .size(14)
            .font(bold())
            .style(text::danger)
            .into()
    } else {
        text("All changes applied")
            .size(14)
            .style(text::secondary)
            .into()
    }
}

fn view_status<'a>(state: &'a State, status: &'a Status) -> Element<'a, Message> {
    let style: fn(&Theme) -> text::Style = if status.error {
        text::danger
    } else {
        text::secondary
    };

    button(text(status.text.as_str()).size(13).style(style))
        .style(button::text)
        .padding([6, 10])
        .on_press_maybe(when(state, true, Message::StatusClear))
        .into()
}

fn view_error<'a>(state: &'a State, error: &'a str) -> Element<'a, Message> {
    let mut content = column![
        text("Could not load the hosts file").size(18),
        text(error).size(13).style(text::danger),
        text("Fix /etc/hosts by hand, or restore a backup.")
            .size(12)
            .style(text::secondary),
        row![
            primary_action("Reload", Some(Message::Reload)),
            action("Restore", Some(Message::RestoreOpen)),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .align_x(Alignment::Center);

    // Sem a lista de grupos, o rodapé não existe: a faixa de status vem para cá.
    if let Some(status) = state.status() {
        content = content.push(view_status(state, status));
    }

    container(content)
        .padding(24)
        .max_width(560.0)
        .center(Length::Fill)
        .into()
}

fn view_no_block() -> Element<'static, Message> {
    let content = column![
        text("No managed block yet").size(18),
        text("Rust Hosts keeps your entries in a marked block at the end of /etc/hosts.")
            .style(text::secondary),
        row![
            primary_action("Create block", Some(Message::CreateBlock)),
            action("Import", Some(Message::ImportOpen)),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .align_x(Alignment::Center);

    container(content)
        .padding(24)
        .max_width(560.0)
        .center(Length::Fill)
        .into()
}

fn view_modal<'a>(modal: &'a Modal) -> Element<'a, Message> {
    let card: Element<'a, Message> = match modal {
        Modal::None => Space::new().into(),
        Modal::Entry(form) => entry_modal(form),
        Modal::Rename(form) => rename_modal(form),
        Modal::Import(form) => import_modal(form),
        Modal::Restore(form) => restore_modal(form),
        Modal::Diff(pending) => diff_modal(pending),
        Modal::Confirm(confirm) => confirm_modal(confirm),
        Modal::Conflict => conflict_modal(),
    };

    container(
        container(card)
            .padding(20)
            .width(Length::Fill)
            .max_width(640.0)
            .style(card_style),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(backdrop_style)
    .into()
}

fn entry_modal(form: &EntryForm) -> Element<'_, Message> {
    let title = if form.target.is_some() {
        "Edit entry"
    } else {
        "Add entry"
    };

    let mut content = column![
        text(title).size(18),
        field(
            "IP address",
            text_input("10.0.0.1", form.ip.as_str())
                .on_input(Message::FormIp)
                .on_submit(Message::FormSubmit),
        ),
        field(
            "Hostnames (space separated)",
            text_input(
                "host.example.com alias.example.com",
                form.hostnames.as_str()
            )
            .on_input(Message::FormHostnames)
            .on_submit(Message::FormSubmit),
        ),
        field(
            "Group",
            // Lista fechada: um typo não pode criar um grupo sem aviso (+ Group cria).
            pick_list(
                form.groups.as_slice(),
                Some(form.group.clone()),
                Message::FormGroup,
            )
            .width(Length::Fill),
        ),
    ]
    .spacing(12);

    if let Some(error) = &form.error {
        content = content.push(text(error.as_str()).size(12).style(text::danger));
    }

    content
        .push(
            row![
                Space::new().width(Length::Fill),
                action("Cancel", Some(Message::FormCancel)),
                primary_action("Save", Some(Message::FormSubmit)),
            ]
            .spacing(8),
        )
        .into()
}

fn rename_modal(form: &RenameForm) -> Element<'_, Message> {
    let (title, submit) = if form.target.is_some() {
        ("Rename group", "Save")
    } else {
        ("New group", "Create")
    };

    let mut content = column![
        text(title).size(18),
        field(
            "Name",
            text_input("Group name", form.name.as_str())
                .on_input(Message::RenameName)
                .on_submit(Message::RenameSubmit),
        ),
    ]
    .spacing(12);

    if let Some(error) = &form.error {
        content = content.push(text(error.as_str()).size(12).style(text::danger));
    }

    content
        .push(
            row![
                Space::new().width(Length::Fill),
                action("Cancel", Some(Message::FormCancel)),
                primary_action(submit, Some(Message::RenameSubmit)),
            ]
            .spacing(8),
        )
        .into()
}

fn import_modal(form: &ImportForm) -> Element<'_, Message> {
    let mut list = column![].spacing(6);
    for (index, candidate) in form.candidates.iter().enumerate() {
        let mut label = format!(
            "{} · {} {}",
            candidate.group.as_deref().unwrap_or(import::UNGROUPED),
            candidate.entry.ip,
            candidate.entry.hostnames.join(" ")
        );
        if candidate.is_system() {
            label.push_str(" (system)");
        }

        list = list.push(
            checkbox(form.checked.get(index).copied().unwrap_or(false))
                .label(label)
                .on_toggle(move |_| Message::ImportToggle(index))
                .text_size(13),
        );
    }

    column![
        text("Import unmanaged entries").size(18),
        text("Selected lines move into the managed block.")
            .size(12)
            .style(text::secondary),
        scrollable(list).height(Length::Fixed(320.0)),
        row![
            Space::new().width(Length::Fill),
            action("Cancel", Some(Message::FormCancel)),
            primary_action("Import", Some(Message::ImportConfirm)),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}

fn restore_modal(form: &RestoreForm) -> Element<'_, Message> {
    let mut list = column![].spacing(4);
    for (index, backup) in form.backups.iter().enumerate() {
        let selected = form.selected == Some(index);
        let label = format!("{} · {}", timestamp(backup), format_size(backup.size));

        list = list.push(
            button(text(label).size(13))
                .style(backup_style(selected))
                .width(Length::Fill)
                .padding([6, 10])
                .on_press(Message::RestorePick(index)),
        );
    }

    column![
        text("Restore backup").size(18),
        text("Restoring replaces the whole hosts file.")
            .size(12)
            .style(text::secondary),
        scrollable(list).height(Length::Fixed(300.0)),
        row![
            Space::new().width(Length::Fill),
            action("Cancel", Some(Message::FormCancel)),
            primary_action(
                "Preview diff",
                form.selected.map(|_| Message::RestoreConfirm),
            ),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}

fn diff_modal(pending: &Pending) -> Element<'_, Message> {
    let sides = row![
        diff_side(
            "Current",
            pending.before.as_str(),
            pending.after.as_str(),
            false
        ),
        diff_side("New", pending.after.as_str(), pending.before.as_str(), true),
    ]
    .spacing(12);

    let mut buttons = row![
        Space::new().width(Length::Fill),
        action("Close", Some(Message::DiffCancel)),
    ]
    .spacing(8);
    if pending.write.is_some() {
        buttons = buttons.push(primary_action("Apply", Some(Message::DiffConfirm)));
    }

    column![
        text(pending.title.as_str()).size(18),
        text("Highlighted lines differ from the other side; everything else is unchanged.")
            .size(12)
            .style(text::secondary),
        sides,
        buttons,
    ]
    .spacing(12)
    .into()
}

/// Comparação por conjunto, não por posição: a pergunta é "esta linha muda?".
///
/// shortcut: linha reposicionada ou duplicada aparece como inalterada, não há diff real.
fn marked_lines<'a>(content: &'a str, other: &'a str) -> Vec<(&'a str, bool)> {
    let other: HashSet<&str> = other.lines().collect();
    content
        .lines()
        .map(|line| (line, !other.contains(line)))
        .collect()
}

fn diff_side<'a>(
    label: &'a str,
    content: &'a str,
    other: &'a str,
    added: bool,
) -> Element<'a, Message> {
    let mut lines = column![].spacing(1);
    for (line, changed) in marked_lines(content, other) {
        // Linha vazia precisa de altura, senão o espaçamento some.
        let shown = if line.is_empty() { " " } else { line };
        lines = lines.push(
            container(
                text(shown)
                    .font(Font::MONOSPACE)
                    .size(12)
                    .width(Length::Fill),
            )
            .padding([1, 6])
            .width(Length::Fill)
            .style(move |theme| line_style(theme, changed, added)),
        );
    }
    if content.is_empty() {
        lines = lines.push(text("(empty)").size(12).style(text::secondary));
    }

    column![
        text(label).size(12).style(text::secondary),
        scrollable(container(lines).padding(6).width(Length::Fill))
            .height(Length::Fixed(340.0))
            .width(Length::Fill),
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn line_style(theme: &Theme, changed: bool, added: bool) -> container::Style {
    if !changed {
        return container::Style::default();
    }

    let palette = theme.extended_palette();
    let color = if added {
        palette.success.weak.color
    } else {
        palette.danger.weak.color
    };

    container::Style {
        background: Some(color.into()),
        ..container::Style::default()
    }
}

fn confirm_modal(confirm: &Confirm) -> Element<'_, Message> {
    let message = match confirm {
        Confirm::DeleteEntry(..) => "Delete this entry?",
        Confirm::DeleteGroup(_) => "Delete this group?",
        Confirm::Reload => "Reload and discard unsaved changes?",
        Confirm::Close(_) => "Close and discard unsaved changes?",
    };

    column![
        text(message).size(16),
        row![
            Space::new().width(Length::Fill),
            action("Cancel", Some(Message::ConfirmNo)),
            danger_action("Confirm", Message::ConfirmYes),
        ]
        .spacing(8),
    ]
    .spacing(16)
    .into()
}

fn conflict_modal() -> Element<'static, Message> {
    column![
        text("The hosts file changed on disk since it was loaded.").size(16),
        text("Reload to continue with the current file, or review the differences first.")
            .size(12)
            .style(text::secondary),
        row![
            Space::new().width(Length::Fill),
            action("Cancel", Some(Message::ConflictCancel)),
            action("Show diff", Some(Message::ConflictDiff)),
            primary_action("Reload", Some(Message::ConflictReload)),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .into()
}

fn field<'a>(label: &'a str, input: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    let input: Element<'a, Message> = input.into();
    column![text(label).size(12).style(text::secondary), input]
        .spacing(4)
        .into()
}

fn action<'a>(label: &'a str, message: Option<Message>) -> Element<'a, Message> {
    button(text(label).size(13))
        .style(button::secondary)
        .padding([6, 12])
        .on_press_maybe(message)
        .into()
}

/// Ação terceira (linhas e cabeçalhos): só texto, para a tela ter um único botão cheio.
fn quiet_action<'a>(label: &'a str, message: Option<Message>) -> Element<'a, Message> {
    button(text(label).size(12))
        .style(quiet_style)
        .padding([6, 8])
        .on_press_maybe(message)
        .into()
}

fn quiet_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let text_color = match status {
        button::Status::Disabled => palette.background.base.text.scale_alpha(0.4),
        button::Status::Hovered | button::Status::Pressed => {
            palette.primary.strong.color.scale_alpha(0.8)
        }
        button::Status::Active => palette.primary.strong.color,
    };

    button::Style {
        text_color,
        ..button::Style::default()
    }
}

fn primary_action<'a>(label: &'a str, message: Option<Message>) -> Element<'a, Message> {
    button(text(label).size(13))
        .style(button::primary)
        .padding([8, 16])
        .on_press_maybe(message)
        .into()
}

fn danger_action<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    button(text(label).size(13))
        .style(button::danger)
        .padding([8, 16])
        .on_press(message)
        .into()
}

/// Nada de segunda escrita nem de edição enquanto o helper roda.
fn when(state: &State, enabled: bool, message: Message) -> Option<Message> {
    (enabled && !state.is_busy()).then_some(message)
}

fn on_toggle(state: &State, enabled: bool, message: Message) -> Option<impl Fn(bool) -> Message> {
    when(state, enabled, message).map(|message| move |_| message.clone())
}

/// Entrada desabilitada se distingue sem depender só de cor.
fn dimmed(enabled: bool) -> impl Fn(&Theme) -> text::Style + Copy {
    move |theme| text::Style {
        color: Some(if enabled {
            theme.palette().text
        } else {
            Color {
                a: 0.5,
                ..theme.palette().text
            }
        }),
    }
}

fn backup_style(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        if selected {
            button::primary(theme, status)
        } else {
            button::secondary(theme, status)
        }
    }
}

fn card_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.base.color.into()),
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }
}

fn backdrop_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.55).into()),
        ..container::Style::default()
    }
}

fn bold() -> Font {
    Font {
        weight: Weight::Bold,
        ..Font::DEFAULT
    }
}

fn timestamp(backup: &Backup) -> String {
    backup.modified.format(STAMP).unwrap_or_default()
}

fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{size} B")
    } else {
        format!("{:.1} KiB", size as f64 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::marked_lines;

    #[test]
    fn diff_marks_only_lines_missing_on_the_other_side() {
        let before = "# BEGIN rust-hosts\n127.0.1.1 old.test\n# END rust-hosts\n";
        let after = "# BEGIN rust-hosts\n127.0.1.1 old.test\n10.0.0.5 new.test\n# END rust-hosts\n";

        let marked: Vec<&str> = marked_lines(after, before)
            .into_iter()
            .filter(|(_, changed)| *changed)
            .map(|(line, _)| line)
            .collect();

        assert_eq!(marked, ["10.0.0.5 new.test"]);
    }
}
