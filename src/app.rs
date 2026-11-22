//! Estado, mensagens e atualização da UI, no padrão Elm do Iced.

use std::net::IpAddr;
use std::path::PathBuf;

use iced::Element;
use iced::{Subscription, Task};

use crate::Error;
use crate::apply::{self, Failure};
use crate::backup::{self, Backup};
use crate::block::{Block, Document, Entry, Group};
use crate::hosts;
use crate::import::{self, Candidate};

mod view;

/// A GUI nunca recebe outro caminho: só o helper escreve, e só aqui.
pub const HOSTS_PATH: &str = "/etc/hosts";

pub fn run() -> iced::Result {
    iced::application(State::load, update, view)
        .title("Rust Hosts")
        .window_size((900, 600))
        .exit_on_close_request(false)
        .subscription(subscription)
        .run()
}

fn view(state: &State) -> Element<'_, Message> {
    view::view(state)
}

fn subscription(_state: &State) -> Subscription<Message> {
    Subscription::batch([
        iced::window::close_requests().map(Message::CloseRequested),
        // Esc fecha o modal aberto; Ctrl+S abre o diff. Ambos passam pelo update,
        // que ignora enquanto a escrita privilegiada está em curso.
        iced::keyboard::listen().filter_map(|event| match event {
            iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            } => Some(Message::FormCancel),
            iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Character(character),
                modifiers,
                ..
            } if modifiers.command() && character.as_str().eq_ignore_ascii_case("s") => {
                Some(Message::ApplyPressed)
            }
            _ => None,
        }),
    ])
}

pub struct State {
    hosts_path: PathBuf,
    backups_dir: PathBuf,
    /// Última leitura do disco; o hash dela é o que o helper confere.
    loaded: String,
    hash: String,
    /// Bloco em edição.
    document: Document,
    /// Como o arquivo chegou do disco: é o que define "dirty".
    baseline: Document,
    load_error: Option<String>,
    search: String,
    /// Grupos fechados na lista, por nome.
    collapsed: Vec<String>,
    unmanaged_open: bool,
    modal: Modal,
    busy: bool,
    /// Estado do helper instalado, checado na abertura e ao rechecar.
    helper: apply::HelperState,
    status: Option<Status>,
}

pub struct Status {
    pub text: String,
    pub error: bool,
}

impl Status {
    fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: false,
        }
    }

    fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: true,
        }
    }
}

pub enum Modal {
    None,
    Entry(EntryForm),
    Rename(RenameForm),
    Import(ImportForm),
    Restore(RestoreForm),
    Diff(Pending),
    Confirm(Confirm),
    /// O arquivo mudou no disco: Apply está bloqueado (design.md, "Conflito externo").
    Conflict,
}

pub struct EntryForm {
    /// `Some` quando está editando uma entrada existente.
    pub target: Option<(usize, usize)>,
    /// Nomes dos grupos existentes, para o formulário não inventar um por typo.
    pub groups: Vec<String>,
    pub group: String,
    pub ip: String,
    pub hostnames: String,
    pub error: Option<String>,
}

pub struct RenameForm {
    /// `None` ao criar: o grupo só entra no documento no submit.
    pub target: Option<usize>,
    pub name: String,
    pub error: Option<String>,
}

pub struct ImportForm {
    pub candidates: Vec<Candidate>,
    pub checked: Vec<bool>,
}

pub struct RestoreForm {
    pub backups: Vec<Backup>,
    pub selected: Option<usize>,
    pub content: Option<String>,
}

/// Diff prestes a ser confirmado. `write: None` é diff só para leitura.
pub struct Pending {
    pub title: String,
    pub before: String,
    pub after: String,
    pub write: Option<Write>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Write {
    Block,
    Full,
}

pub enum Confirm {
    DeleteEntry(usize, usize),
    DeleteGroup(usize),
    Reload,
    Close(iced::window::Id),
}

#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    ToggleUnmanaged,
    CreateBlock,
    ToggleGroup(usize),
    ToggleGroupCollapsed(usize),
    ToggleAll,
    NewGroup,
    AddEntry(usize),
    RenameGroup(usize),
    DeleteGroup(usize),
    ToggleEntry(usize, usize),
    EditEntry(usize, usize),
    DeleteEntry(usize, usize),
    FormIp(String),
    FormHostnames(String),
    FormGroup(String),
    FormSubmit,
    FormCancel,
    RenameName(String),
    RenameSubmit,
    ImportOpen,
    ImportToggle(usize),
    ImportConfirm,
    RestoreOpen,
    RestorePick(usize),
    RestoreConfirm,
    DiffConfirm,
    DiffCancel,
    ApplyPressed,
    WriteFinished(Result<(), Failure>),
    ConflictReload,
    ConflictDiff,
    ConflictCancel,
    Reload,
    ConfirmYes,
    ConfirmNo,
    CheckHelper,
    InstallHelper,
    Installed(Result<(), Failure>),
    CopyCommand,
    StatusClear,
    CloseRequested(iced::window::Id),
}

impl State {
    fn load() -> Self {
        let mut state = Self {
            hosts_path: PathBuf::from(HOSTS_PATH),
            backups_dir: backup::default_dir(),
            loaded: String::new(),
            hash: String::new(),
            document: Document::default(),
            baseline: Document::default(),
            load_error: None,
            search: String::new(),
            collapsed: Vec::new(),
            unmanaged_open: false,
            modal: Modal::None,
            busy: false,
            helper: apply::helper_state(),
            status: None,
        };

        if let Err(error) = state.read_hosts() {
            state.load_error = Some(error);
        }
        state
    }

    fn read_hosts(&mut self) -> Result<(), String> {
        let text = hosts::read(&self.hosts_path).map_err(|error| error.to_string())?;
        let document = Document::parse(&text).map_err(|error| error.to_string())?;

        self.hash = hosts::hash(text.as_bytes());
        self.document = document.clone();
        self.baseline = document;
        self.loaded = text;
        self.load_error = None;
        Ok(())
    }

    pub fn is_dirty(&self) -> bool {
        self.document != self.baseline
    }

    pub fn load_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn helper(&self) -> apply::HelperState {
        self.helper
    }

    pub fn helper_ready(&self) -> bool {
        self.helper == apply::HelperState::Ready
    }

    /// Binário que acompanha o app, usado para instalar o helper.
    pub fn helper_source(&self) -> Option<PathBuf> {
        apply::helper_source()
    }

    pub fn status(&self) -> Option<&Status> {
        self.status.as_ref()
    }

    pub fn modal(&self) -> &Modal {
        &self.modal
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn loaded(&self) -> &str {
        &self.loaded
    }

    pub fn search(&self) -> &str {
        &self.search
    }

    pub fn unmanaged_open(&self) -> bool {
        self.unmanaged_open
    }

    pub fn is_collapsed(&self, group: &Group) -> bool {
        self.collapsed.iter().any(|name| name == &group.name)
    }

    /// Grupos vazios continuam visíveis mesmo sem busca.
    fn group_name_matches(&self, group: &Group) -> bool {
        let query = self.search.trim().to_lowercase();
        query.is_empty() || group.name.to_lowercase().contains(&query)
    }

    fn entry_matches(&self, entry: &Entry) -> bool {
        let query = self.search.trim().to_lowercase();
        query.is_empty()
            || entry.ip.to_string().contains(&query)
            || entry
                .hostnames
                .iter()
                .any(|hostname| hostname.to_lowercase().contains(&query))
    }

    pub fn is_visible(&self, group: &Group) -> bool {
        self.group_name_matches(group)
            || group.entries.iter().any(|entry| self.entry_matches(entry))
    }

    /// Grupo que casa pelo nome mostra todas as entradas; senão, só as que casam.
    pub fn visible_entries<'a>(&self, group: &'a Group) -> Vec<(usize, &'a Entry)> {
        let all = self.group_name_matches(group);
        group
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| all || self.entry_matches(entry))
            .collect()
    }

    pub fn group(&self, index: usize) -> Option<&Group> {
        self.document.block.as_ref()?.groups.get(index)
    }

    fn group_mut(&mut self, index: usize) -> Option<&mut Group> {
        self.document.block.as_mut()?.groups.get_mut(index)
    }

    fn entry_mut(&mut self, group: usize, entry: usize) -> Option<&mut Entry> {
        self.document
            .block
            .as_mut()?
            .groups
            .get_mut(group)?
            .entries
            .get_mut(entry)
    }

    /// Uma entrada habilitada entre as do grupo decide o toggle de grupo.
    fn toggle_group(&mut self, index: usize) {
        let Some(group) = self.group_mut(index) else {
            return;
        };
        let disable = group.entries.iter().all(|entry| entry.enabled);
        for entry in &mut group.entries {
            entry.enabled = !disable;
        }
    }

    fn toggle_all(&mut self) {
        let enable = !self.all_enabled();
        if let Some(block) = self.document.block.as_mut() {
            for entry in block.groups.iter_mut().flat_map(|group| &mut group.entries) {
                entry.enabled = enable;
            }
        }
    }

    pub fn has_entries(&self) -> bool {
        self.document
            .block
            .as_ref()
            .is_some_and(|block| block.groups.iter().any(|group| !group.entries.is_empty()))
    }

    pub fn all_enabled(&self) -> bool {
        self.document.block.as_ref().is_some_and(|block| {
            block
                .groups
                .iter()
                .flat_map(|group| &group.entries)
                .all(|entry| entry.enabled)
        })
    }

    fn toggle_collapsed(&mut self, index: usize) {
        let Some(name) = self.group(index).map(|group| group.name.clone()) else {
            return;
        };
        match self
            .collapsed
            .iter()
            .position(|collapsed| collapsed == &name)
        {
            Some(position) => {
                self.collapsed.remove(position);
            }
            None => self.collapsed.push(name),
        }
    }

    fn open_entry(&mut self, group: usize, target: Option<(usize, usize)>) {
        let Some(current) = self.group(group) else {
            return;
        };

        let (ip, hostnames) = target
            .and_then(|(group, entry)| {
                self.group(group)
                    .and_then(|group| group.entries.get(entry))
                    .map(|entry| (entry.ip.to_string(), entry.hostnames.join(" ")))
            })
            .unwrap_or_default();

        self.modal = Modal::Entry(EntryForm {
            target,
            groups: self
                .document
                .block
                .as_ref()
                .map(|block| {
                    block
                        .groups
                        .iter()
                        .map(|group| group.name.clone())
                        .collect()
                })
                .unwrap_or_default(),
            group: current.name.clone(),
            ip,
            hostnames,
            error: None,
        });
    }

    fn submit_entry(&mut self) {
        let Modal::Entry(form) = &self.modal else {
            return;
        };

        let ip = match form.ip.trim().parse::<IpAddr>() {
            Ok(ip) => ip,
            Err(_) => {
                if let Modal::Entry(form) = &mut self.modal {
                    form.error = Some(Error::InvalidIp(form.ip.trim().to_owned()).to_string());
                }
                return;
            }
        };

        let hostnames: Vec<String> = form
            .hostnames
            .split(|character: char| character.is_whitespace() || character == ',')
            .filter(|hostname| !hostname.is_empty())
            .map(str::to_owned)
            .collect();

        let enabled = form
            .target
            .and_then(|(group, entry)| self.group(group)?.entries.get(entry))
            .map(|entry| entry.enabled)
            .unwrap_or(true);

        let (target, name, entry) = match (
            Entry::new(ip, hostnames, enabled),
            Group::new(form.group.clone()),
        ) {
            (Ok(entry), Ok(group)) => (form.target, group.name, entry),
            (Err(error), _) | (_, Err(error)) => {
                if let Modal::Entry(form) = &mut self.modal {
                    form.error = Some(error.to_string());
                }
                return;
            }
        };

        if let Some((group, index)) = target
            && self
                .group(group)
                .is_some_and(|current| current.name == name)
            && let Some(slot) = self.entry_mut(group, index)
        {
            // Mesma entrada no mesmo grupo: troca no lugar, sem reordenar a lista.
            *slot = entry;
            self.modal = Modal::None;
            return;
        }

        if let Some((group, index)) = target
            && let Some(entries) = self
                .document
                .block
                .as_mut()
                .and_then(|block| block.groups.get_mut(group))
                .map(|group| &mut group.entries)
            && index < entries.len()
        {
            entries.remove(index);
        }

        let block = self.document.block.get_or_insert_with(Block::default);
        let index = match block.groups.iter().position(|group| group.name == name) {
            Some(index) => index,
            None => {
                block.groups.push(Group {
                    name,
                    entries: Vec::new(),
                });
                block.groups.len() - 1
            }
        };
        block.groups[index].entries.push(entry);
        self.modal = Modal::None;
    }

    fn submit_rename(&mut self) {
        let Modal::Rename(form) = &self.modal else {
            return;
        };

        let target = form.target;
        let name = match Group::new(form.name.clone()) {
            Ok(group) => group.name,
            Err(error) => {
                if let Modal::Rename(form) = &mut self.modal {
                    form.error = Some(error.to_string());
                }
                return;
            }
        };

        let taken = self.document.block.as_ref().is_some_and(|block| {
            block
                .groups
                .iter()
                .enumerate()
                .any(|(index, group)| Some(index) != target && group.name == name)
        });
        if taken {
            if let Modal::Rename(form) = &mut self.modal {
                form.error = Some(Error::DuplicateGroup(name).to_string());
            }
            return;
        }

        match target {
            Some(index) => {
                if let Some(group) = self.group_mut(index) {
                    group.name = name;
                }
            }
            None => {
                if let Some(block) = self.document.block.as_mut() {
                    block.groups.push(Group {
                        name,
                        entries: Vec::new(),
                    });
                }
                // Um filtro ativo esconderia o grupo recém-criado.
                self.search.clear();
            }
        }
        self.modal = Modal::None;
    }

    fn open_import(&mut self) {
        let candidates = import::scan(&self.document);
        if candidates.is_empty() {
            self.status = Some(Status::error("No unmanaged entries to import"));
            return;
        }
        let checked = candidates.iter().map(|entry| !entry.is_system()).collect();
        self.modal = Modal::Import(ImportForm {
            candidates,
            checked,
        });
    }

    fn confirm_import(&mut self) {
        let Modal::Import(form) = &self.modal else {
            return;
        };
        let selected: Vec<Candidate> = form
            .candidates
            .iter()
            .zip(&form.checked)
            .filter(|(_, checked)| **checked)
            .map(|(candidate, _)| candidate.clone())
            .collect();

        if selected.is_empty() {
            self.status = Some(Status::error("Nothing selected"));
            return;
        }

        let pending = Pending {
            title: "Import entries".to_owned(),
            before: self.loaded.clone(),
            after: import::import(&self.document, &selected),
            write: Some(Write::Full),
        };
        self.begin_write(pending);
    }

    fn open_restore(&mut self) {
        match backup::list(&self.backups_dir) {
            Ok(backups) if backups.is_empty() => {
                self.status = Some(Status::error("No backups yet"));
            }
            Ok(backups) => {
                self.modal = Modal::Restore(RestoreForm {
                    backups,
                    selected: None,
                    content: None,
                });
            }
            Err(error) => self.status = Some(Status::error(error.to_string())),
        }
    }

    fn pick_restore(&mut self, index: usize) {
        let Modal::Restore(form) = &mut self.modal else {
            return;
        };
        let Some(backup) = form.backups.get(index) else {
            return;
        };
        match hosts::read(&backup.path) {
            Ok(content) => {
                form.selected = Some(index);
                form.content = Some(content);
            }
            Err(error) => self.status = Some(Status::error(error.to_string())),
        }
    }

    fn confirm_restore(&mut self) {
        let Modal::Restore(form) = &self.modal else {
            return;
        };
        let Some(content) = form.content.clone() else {
            self.status = Some(Status::error("Pick a backup first"));
            return;
        };

        // Com erro de leitura o estado guarda hash vazio: sem reler o arquivo,
        // o Restore cairia sempre em "mudou no disco" e nunca recuperaria nada.
        if self.load_error.is_some()
            && let Ok(text) = hosts::read(&self.hosts_path)
        {
            self.hash = hosts::hash(text.as_bytes());
            self.loaded = text;
        }

        let pending = Pending {
            title: "Restore backup".to_owned(),
            before: self.loaded.clone(),
            after: content,
            write: Some(Write::Full),
        };
        self.begin_write(pending);
    }

    /// Qualquer escrita reconfere o arquivo: nunca sobrescrever em silêncio.
    fn begin_write(&mut self, pending: Pending) {
        match hosts::read(&self.hosts_path) {
            Ok(text) if hosts::hash(text.as_bytes()) == self.hash => {
                self.modal = Modal::Diff(pending);
            }
            Ok(_) => self.modal = Modal::Conflict,
            Err(error) => self.status = Some(Status::error(error.to_string())),
        }
    }

    fn apply_pressed(&mut self) {
        // Ctrl+S chega aqui sem passar pelos gates do `when` da view.
        if self.busy || !matches!(self.modal, Modal::None) {
            return;
        }
        if self.document.block.is_none() {
            self.status = Some(Status::error("There is no managed block to apply"));
            return;
        }
        // Write idêntico gastaria uma vaga de backup e pediria senha à toa.
        if !self.is_dirty() {
            self.status = Some(Status::ok("No changes to apply"));
            return;
        }
        let pending = Pending {
            title: "Apply managed block".to_owned(),
            before: self.baseline.block_text(),
            after: self.document.block_text(),
            write: Some(Write::Block),
        };
        self.begin_write(pending);
    }

    fn start_write(&mut self, pending: Pending) -> Task<Message> {
        let Some(write) = pending.write else {
            return Task::none();
        };

        // O backup é a única recuperação se a escrita der errado.
        if let Err(error) = backup::create(&self.backups_dir, self.loaded.as_bytes()) {
            self.status = Some(Status::error(format!("Backup failed: {error}")));
            return Task::none();
        }

        self.busy = true;
        let hash = self.hash.clone();
        let input = pending.after;

        // O executor padrão do Iced é um thread pool: bloquear aqui não trava a UI.
        Task::perform(
            async move {
                match write {
                    Write::Block => apply::apply_block(&input, &hash),
                    Write::Full => apply::write_full(&input, &hash),
                }
            },
            Message::WriteFinished,
        )
    }

    fn reload(&mut self) {
        match self.read_hosts() {
            Ok(()) => self.status = Some(Status::ok("Reloaded")),
            Err(error) => self.load_error = Some(error),
        }
    }

    fn confirm_yes(&mut self) -> Task<Message> {
        let Modal::Confirm(confirm) = std::mem::replace(&mut self.modal, Modal::None) else {
            return Task::none();
        };

        match confirm {
            Confirm::DeleteEntry(group, entry) => {
                if let Some(group) = self.group_mut(group)
                    && entry < group.entries.len()
                {
                    group.entries.remove(entry);
                }
                Task::none()
            }
            Confirm::DeleteGroup(index) => {
                if let Some(block) = self.document.block.as_mut()
                    && block
                        .groups
                        .get(index)
                        .is_some_and(|group| group.entries.is_empty())
                {
                    block.groups.remove(index);
                }
                Task::none()
            }
            Confirm::Reload => {
                self.reload();
                Task::none()
            }
            Confirm::Close(window) => iced::window::close(window),
        }
    }
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::SearchChanged(value) => state.search = value,
        Message::ToggleUnmanaged => state.unmanaged_open = !state.unmanaged_open,
        Message::CreateBlock => {
            if state.document.block.is_none() {
                state.document.block = Some(Block::default());
            }
        }
        Message::ToggleGroup(index) => state.toggle_group(index),
        Message::ToggleGroupCollapsed(index) => state.toggle_collapsed(index),
        Message::ToggleAll => state.toggle_all(),
        // Bloco vazio sem grupo nenhum não teria por onde começar além do Import.
        Message::NewGroup => {
            if state.document.block.is_some() {
                // O grupo só entra no documento no submit: cancelar não pode deixar lixo.
                state.modal = Modal::Rename(RenameForm {
                    target: None,
                    name: String::new(),
                    error: None,
                });
            }
        }
        Message::AddEntry(index) => state.open_entry(index, None),
        Message::EditEntry(group, entry) => state.open_entry(group, Some((group, entry))),
        Message::RenameGroup(index) => {
            if let Some(group) = state.group(index) {
                state.modal = Modal::Rename(RenameForm {
                    target: Some(index),
                    name: group.name.clone(),
                    error: None,
                });
            }
        }
        Message::DeleteGroup(index) => match state.group(index) {
            Some(group) if group.entries.is_empty() => {
                state.modal = Modal::Confirm(Confirm::DeleteGroup(index));
            }
            Some(_) => {
                state.status = Some(Status::error(
                    "Only empty groups can be deleted; remove the entries first",
                ));
            }
            None => {}
        },
        Message::ToggleEntry(group, entry) => {
            if let Some(entry) = state.entry_mut(group, entry) {
                entry.enabled = !entry.enabled;
            }
        }
        Message::DeleteEntry(group, entry) => {
            state.modal = Modal::Confirm(Confirm::DeleteEntry(group, entry));
        }
        Message::FormIp(value) => {
            if let Modal::Entry(form) = &mut state.modal {
                form.ip = value;
                form.error = None;
            }
        }
        Message::FormHostnames(value) => {
            if let Modal::Entry(form) = &mut state.modal {
                form.hostnames = value;
                form.error = None;
            }
        }
        Message::FormGroup(value) => {
            if let Modal::Entry(form) = &mut state.modal {
                form.group = value;
                form.error = None;
            }
        }
        Message::FormSubmit => state.submit_entry(),
        Message::FormCancel => {
            // Esc/Cancel não pode abortar uma escrita já autorizada.
            if !state.busy {
                state.modal = Modal::None;
            }
        }
        Message::RenameName(value) => {
            if let Modal::Rename(form) = &mut state.modal {
                form.name = value;
                form.error = None;
            }
        }
        Message::RenameSubmit => state.submit_rename(),
        Message::ImportOpen => state.open_import(),
        Message::ImportToggle(index) => {
            if let Modal::Import(form) = &mut state.modal
                && let Some(checked) = form.checked.get_mut(index)
            {
                *checked = !*checked;
            }
        }
        Message::ImportConfirm => state.confirm_import(),
        Message::RestoreOpen => state.open_restore(),
        Message::RestorePick(index) => state.pick_restore(index),
        Message::RestoreConfirm => state.confirm_restore(),
        Message::DiffConfirm => {
            let Modal::Diff(pending) = std::mem::replace(&mut state.modal, Modal::None) else {
                return Task::none();
            };
            return state.start_write(pending);
        }
        Message::DiffCancel => state.modal = Modal::None,
        Message::ApplyPressed => state.apply_pressed(),
        Message::WriteFinished(result) => {
            state.busy = false;
            match result {
                Ok(()) => {
                    state.reload();
                    state.status = Some(Status::ok("Applied"));
                }
                Err(Failure::Cancelled) => {
                    // 126/127: diálogo dispensado, autorização negada ou helper ausente.
                    state.status = Some(Status::error(
                        "Not authorized: pkexec was cancelled or the helper is not installed",
                    ));
                }
                Err(Failure::Helper { code: 5, .. }) => {
                    state.modal = Modal::Conflict;
                }
                Err(Failure::Helper { message, .. }) => {
                    state.status = Some(Status::error(message));
                }
                Err(Failure::Spawn(message)) => {
                    state.status = Some(Status::error(message));
                }
            }
        }
        Message::ConflictReload => {
            if state.is_dirty() {
                state.modal = Modal::Confirm(Confirm::Reload);
            } else {
                state.reload();
                state.modal = Modal::None;
            }
        }
        Message::ConflictDiff => match hosts::read(&state.hosts_path) {
            Ok(text) => {
                state.modal = Modal::Diff(Pending {
                    title: "File changed on disk".to_owned(),
                    before: state.loaded.clone(),
                    after: text,
                    write: None,
                });
            }
            Err(error) => state.status = Some(Status::error(error.to_string())),
        },
        Message::ConflictCancel => state.modal = Modal::None,
        Message::Reload => {
            if state.is_dirty() {
                state.modal = Modal::Confirm(Confirm::Reload);
            } else {
                state.reload();
            }
        }
        Message::ConfirmYes => return state.confirm_yes(),
        Message::ConfirmNo => state.modal = Modal::None,
        Message::CheckHelper => {
            state.helper = apply::helper_state();
            state.status = Some(match state.helper {
                apply::HelperState::Ready => Status::ok("Helper is up to date"),
                apply::HelperState::Missing => Status::error("Helper is still missing"),
                apply::HelperState::Outdated => Status::error("Helper is still outdated"),
            });
        }
        Message::InstallHelper => {
            let Some(source) = apply::helper_source() else {
                state.status = Some(Status::error(
                    "The helper binary is not next to the app; build it with cargo build --release",
                ));
                return Task::none();
            };
            state.busy = true;
            return Task::perform(
                async move { apply::install_helper(&source) },
                Message::Installed,
            );
        }
        Message::Installed(result) => {
            state.busy = false;
            match result {
                Ok(()) => {
                    state.helper = apply::helper_state();
                    state.status = Some(Status::ok("Helper installed"));
                }
                Err(Failure::Cancelled) => {
                    state.status = Some(Status::error("Installation cancelled"));
                }
                Err(Failure::Helper { message, .. }) => {
                    state.status = Some(Status::error(message));
                }
                Err(Failure::Spawn(message)) => {
                    state.status = Some(Status::error(message));
                }
            }
        }
        Message::CopyCommand => match apply::helper_source() {
            Some(source) => return iced::clipboard::write(apply::install_command(&source)),
            None => {
                state.status = Some(Status::error(
                    "The helper binary is not next to the app; build it with cargo build --release",
                ));
            }
        },
        Message::StatusClear => state.status = None,
        Message::CloseRequested(window) => {
            if state.is_dirty() {
                state.modal = Modal::Confirm(Confirm::Close(window));
            } else {
                return iced::window::close(window);
            }
        }
    }

    Task::none()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
127.0.0.1 localhost
# BEGIN rust-hosts
# Dev
10.0.0.1 dev.test
# 10.0.0.2 off.test
# Prod
10.0.0.3 hml.test
# END rust-hosts
";

    fn send(state: &mut State, message: Message) {
        let _ = update(state, message);
    }

    fn state() -> State {
        let document = Document::parse(SAMPLE).unwrap();
        State {
            // Nunca aponta para o /etc/hosts real: os testes não leem disco.
            hosts_path: PathBuf::from("/nonexistent/rust-hosts-test"),
            backups_dir: PathBuf::from("/nonexistent/rust-hosts-test"),
            loaded: SAMPLE.to_owned(),
            hash: hosts::hash(SAMPLE.as_bytes()),
            document: document.clone(),
            baseline: document,
            load_error: None,
            search: String::new(),
            collapsed: Vec::new(),
            unmanaged_open: false,
            modal: Modal::None,
            busy: false,
            helper: apply::HelperState::Ready,
            status: None,
        }
    }

    #[test]
    fn dirty_only_after_a_real_change() {
        let mut state = state();
        assert!(!state.is_dirty());

        send(&mut state, Message::ToggleEntry(0, 0));

        assert!(state.is_dirty());
    }

    #[test]
    fn cancel_does_not_dismiss_the_modal_while_writing() {
        let mut state = state();
        state.busy = true;
        state.modal = Modal::Conflict;

        send(&mut state, Message::FormCancel);

        assert!(matches!(state.modal(), Modal::Conflict));
    }

    #[test]
    fn restore_recovers_a_file_that_fails_to_parse() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts");
        let broken = "# BEGIN rust-hosts\n10.0.0.1 sem-grupo.test\n# END rust-hosts\n";
        std::fs::write(&path, broken).unwrap();

        let mut state = state();
        state.hosts_path = path;
        state.loaded = String::new();
        state.hash = String::new();
        state.load_error = Some("the managed block is missing, duplicated or malformed".to_owned());
        state.modal = Modal::Restore(RestoreForm {
            backups: Vec::new(),
            selected: Some(0),
            content: Some("127.0.0.1 localhost\n".to_owned()),
        });

        send(&mut state, Message::RestoreConfirm);

        // Diff e não Conflict: o hash foi refeito a partir do arquivo ilegível.
        assert!(matches!(state.modal(), Modal::Diff(_)));
        assert_eq!(state.loaded, broken);
    }

    #[test]
    fn toggling_a_group_enables_every_entry_when_one_is_disabled() {
        let mut state = state();

        send(&mut state, Message::ToggleGroup(0));

        let entries = &state.group(0).unwrap().entries;
        assert!(entries.iter().all(|entry| entry.enabled));
    }

    #[test]
    fn toggling_a_group_disables_every_entry_when_all_are_enabled() {
        let mut state = state();

        send(&mut state, Message::ToggleGroup(1));

        let entries = &state.group(1).unwrap().entries;
        assert!(entries.iter().all(|entry| !entry.enabled));
    }

    #[test]
    fn toggle_all_enables_then_disables_the_whole_block() {
        let mut state = state();
        assert!(!state.all_enabled());

        send(&mut state, Message::ToggleAll);

        assert!(state.all_enabled());
        assert!(state.is_dirty());

        send(&mut state, Message::ToggleAll);

        let entries = state
            .document()
            .block
            .as_ref()
            .unwrap()
            .groups
            .iter()
            .flat_map(|group| &group.entries);
        assert!(entries.clone().all(|entry| !entry.enabled));
        assert_eq!(entries.count(), 3);
    }

    #[test]
    fn a_non_empty_group_cannot_be_deleted() {
        let mut state = state();

        send(&mut state, Message::DeleteGroup(1));

        assert!(state.group(1).is_some());
        assert!(state.status().unwrap().error);
    }

    #[test]
    fn submitting_the_form_adds_the_entry_to_the_named_group() {
        let mut state = state();
        send(&mut state, Message::AddEntry(1));
        send(&mut state, Message::FormIp("10.0.0.7".to_owned()));
        send(
            &mut state,
            Message::FormHostnames("new.test alias.test".to_owned()),
        );

        send(&mut state, Message::FormSubmit);

        let entries = &state.group(1).unwrap().entries;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].hostnames, ["new.test", "alias.test"]);
        assert!(matches!(state.modal(), Modal::None));
    }

    #[test]
    fn submitting_an_invalid_ip_keeps_the_form_open() {
        let mut state = state();
        send(&mut state, Message::AddEntry(0));
        send(&mut state, Message::FormIp("10.0.0".to_owned()));

        send(&mut state, Message::FormSubmit);

        let Modal::Entry(form) = state.modal() else {
            panic!("expected the entry form to stay open");
        };
        assert!(form.error.is_some());
        assert_eq!(state.group(0).unwrap().entries.len(), 2);
    }

    #[test]
    fn editing_inside_the_same_group_keeps_the_position() {
        let mut state = state();
        send(&mut state, Message::EditEntry(0, 0));
        send(&mut state, Message::FormIp("10.0.0.9".to_owned()));

        send(&mut state, Message::FormSubmit);

        let entries = &state.group(0).unwrap().entries;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].ip.to_string(), "10.0.0.9");
        assert_eq!(entries[0].hostnames, ["dev.test"]);
    }

    #[test]
    fn editing_an_entry_moves_it_to_the_chosen_group() {
        let mut state = state();
        send(&mut state, Message::EditEntry(0, 0));
        send(&mut state, Message::FormGroup("Prod".to_owned()));

        send(&mut state, Message::FormSubmit);

        let dev = &state.group(0).unwrap().entries;
        let prod = &state.group(1).unwrap().entries;
        assert_eq!(dev.len(), 1);
        assert_eq!(prod.len(), 2);
        assert!(prod.iter().any(|entry| entry.hostnames == ["dev.test"]));
    }

    #[test]
    fn a_renamed_group_cannot_take_a_marker_name() {
        let mut state = state();
        send(&mut state, Message::RenameGroup(0));
        send(
            &mut state,
            Message::RenameName("BEGIN rust-hosts".to_owned()),
        );

        send(&mut state, Message::RenameSubmit);

        assert_eq!(state.group(0).unwrap().name, "Dev");
        assert!(matches!(state.modal(), Modal::Rename(_)));
    }

    #[test]
    fn cancelling_a_new_group_leaves_no_group_behind() {
        let mut state = state();
        send(&mut state, Message::NewGroup);
        send(&mut state, Message::FormCancel);

        assert!(!state.is_dirty());
        assert_eq!(state.document().block.as_ref().unwrap().groups.len(), 2);
    }

    #[test]
    fn a_new_group_is_added_only_when_named() {
        let mut state = state();
        send(&mut state, Message::NewGroup);
        send(&mut state, Message::RenameName("Ops".to_owned()));
        send(&mut state, Message::RenameSubmit);

        let groups = &state.document().block.as_ref().unwrap().groups;
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[2].name, "Ops");
        assert!(state.is_dirty());
        assert!(matches!(state.modal(), Modal::None));
    }

    #[test]
    fn a_new_group_cannot_take_an_existing_name() {
        let mut state = state();
        send(&mut state, Message::NewGroup);
        send(&mut state, Message::RenameName("Dev".to_owned()));
        send(&mut state, Message::RenameSubmit);

        assert_eq!(state.document().block.as_ref().unwrap().groups.len(), 2);
        assert!(matches!(state.modal(), Modal::Rename(_)));
    }

    #[test]
    fn renaming_to_an_existing_name_is_refused() {
        let mut state = state();
        send(&mut state, Message::RenameGroup(1));
        send(&mut state, Message::RenameName("Dev".to_owned()));
        send(&mut state, Message::RenameSubmit);

        assert_eq!(state.group(1).unwrap().name, "Prod");
        assert!(matches!(state.modal(), Modal::Rename(_)));
    }

    #[test]
    fn keeping_the_same_name_is_not_a_duplicate() {
        let mut state = state();
        send(&mut state, Message::RenameGroup(0));
        send(&mut state, Message::RenameSubmit);

        assert!(matches!(state.modal(), Modal::None));
        assert!(!state.is_dirty());
    }

    #[test]
    fn apply_without_changes_opens_nothing() {
        let mut state = state();
        send(&mut state, Message::ApplyPressed);

        assert!(matches!(state.modal(), Modal::None));
        assert_eq!(state.status().unwrap().text, "No changes to apply");
    }

    #[test]
    fn apply_is_ignored_while_a_modal_is_open() {
        let mut state = state();
        send(&mut state, Message::ToggleEntry(0, 0));
        state.modal = Modal::Confirm(Confirm::Reload);

        send(&mut state, Message::ApplyPressed);

        assert!(matches!(state.modal(), Modal::Confirm(Confirm::Reload)));
    }

    #[test]
    fn search_matches_by_ip_hostname_and_group_name() {
        let mut state = state();

        send(&mut state, Message::SearchChanged("10.0.0.2".to_owned()));
        let dev = state.group(0).unwrap();
        assert!(state.is_visible(dev));
        assert_eq!(state.visible_entries(dev).len(), 1);

        send(&mut state, Message::SearchChanged("prod".to_owned()));
        let dev = state.group(0).unwrap();
        assert!(!state.is_visible(dev));
        let prod = state.group(1).unwrap();
        assert_eq!(state.visible_entries(prod).len(), 1);
    }
}
