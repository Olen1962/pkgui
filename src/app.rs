use crate::i18n::{get_translations, Language, Translations};
use crate::pkg::{self, Package, PackageDetails};
use gtk4::prelude::*;
use gtk4::{
    gio, glib, Align, Application, ApplicationWindow, Box as GtkBox, Button, DropDown, Entry,
    Expander, Frame, Image, Label, LinkButton, ListBox, ListBoxRow, ScrolledWindow, Separator,
    Spinner, ToggleButton,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

/// Máximo de resultados desenhados na pesquisa de "Disponíveis"
/// (o catálogo tem dezenas de milhares de pacotes).
const MAX_RESULTS: usize = 200;
const SEARCH_DELAY_MS: u64 = 200;

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Installed,
    Available,
    Updates,
    Orphans,
}

impl View {
    fn title(self, t: &Translations) -> &'static str {
        match self {
            View::Installed => t.title_installed,
            View::Available => t.title_available,
            View::Updates => t.title_updates,
            View::Orphans => t.title_orphans,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Theme {
    System,
    Light,
    Dark,
}

thread_local! {
    static SYSTEM_DARK: Cell<Option<bool>> = Cell::new(None);
}

/// A aplicação segue o tema GTK do desktop. "Sistema" mantém a preferência
/// original; Claro/Escuro pedem a variante clara/escura do tema ativo
/// (só tem efeito se o tema tiver variante escura).
fn apply_theme(theme: Theme) {
    if let Some(settings) = gtk4::Settings::default() {
        let original = SYSTEM_DARK.with(|c| {
            if c.get().is_none() {
                c.set(Some(settings.is_gtk_application_prefer_dark_theme()));
            }
            c.get().unwrap_or(false)
        });
        settings.set_gtk_application_prefer_dark_theme(match theme {
            Theme::System => original,
            Theme::Light => false,
            Theme::Dark => true,
        });
    }
}

fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("pkgui").join("settings.conf"))
}

fn load_settings() -> (Language, Theme) {
    let mut lang = Language::Portuguese;
    let mut theme = Theme::System;
    if let Some(text) = settings_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        for line in text.lines() {
            match line.trim().split_once('=') {
                Some(("lang", "en")) => lang = Language::English,
                Some(("lang", "pt")) => lang = Language::Portuguese,
                Some(("theme", "light")) => theme = Theme::Light,
                Some(("theme", "dark")) => theme = Theme::Dark,
                Some(("theme", "system")) => theme = Theme::System,
                _ => {}
            }
        }
    }
    (lang, theme)
}

fn persist_settings(st: &Shared) {
    let (lang, theme) = {
        let s = st.borrow();
        (s.lang, s.theme)
    };
    if let Some(path) = settings_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let content = format!(
            "lang={}\ntheme={}\n",
            if lang == Language::English { "en" } else { "pt" },
            match theme {
                Theme::System => "system",
                Theme::Light => "light",
                Theme::Dark => "dark",
            }
        );
        let _ = std::fs::write(path, content);
    }
}

#[derive(Clone, Copy)]
enum Action {
    Install,
    Remove,
    Reinstall,
}

struct State {
    lang: Language,
    theme: Theme,
    view: View,
    /// Lista completa da vista atual (exceto "Disponíveis")
    packages: Vec<Package>,
    selected: Option<String>,
    installed_names: Rc<HashSet<String>>,
    /// Catálogo completo dos repositórios (carregado uma vez)
    available_cache: Option<Rc<Vec<Package>>>,
    loading_available: bool,
    search_gen: u64,
}

type Shared = Rc<RefCell<State>>;

#[derive(Clone)]
struct Ui {
    window: ApplicationWindow,
    search: Entry,
    spinner: Spinner,
    top_actions: GtkBox,
    sidebar: GtkBox,
    list: ListBox,
    orphans_bar: GtkBox,
    remove_orphans_btn: Button,
    refresh_btn: Button,
    update_all_btn: Button,
    upgrade_all_btn: Button,
    installed_btn: Button,
    available_btn: Button,
    updates_btn: Button,
    orphans_btn: Button,
    install_btn: Button,
    remove_btn: Button,
    reinstall_btn: Button,
    prefs_btn: Button,
    quit_btn: Button,
    busy: Rc<Cell<u32>>,
    locks: Rc<Cell<u32>>,
    suppress_search: Rc<Cell<bool>>,
}

impl Ui {
    fn begin(&self, lock: bool) {
        self.busy.set(self.busy.get() + 1);
        if lock {
            self.locks.set(self.locks.get() + 1);
        }
        self.refresh_busy();
    }

    fn end(&self, lock: bool) {
        self.busy.set(self.busy.get().saturating_sub(1));
        if lock {
            self.locks.set(self.locks.get().saturating_sub(1));
        }
        self.refresh_busy();
    }

    /// Ampulheta do rato + spinner + bloqueio dos botões durante operações.
    fn refresh_busy(&self) {
        let busy = self.busy.get() > 0;
        let locked = self.locks.get() > 0;
        self.spinner.set_spinning(busy);
        let cursor = if locked {
            Some("wait") // ampulheta
        } else if busy {
            Some("progress") // seta + ampulheta (não bloqueia)
        } else {
            None
        };
        self.window.set_cursor_from_name(cursor);
        self.sidebar.set_sensitive(!locked);
        self.top_actions.set_sensitive(!locked);
        self.orphans_bar.set_sensitive(!locked);
    }

    fn apply_language(&self, t: &Translations) {
        self.search.set_placeholder_text(Some(t.search_placeholder));
        self.refresh_btn.set_label(t.refresh);
        self.update_all_btn.set_label(t.update_all);
        self.upgrade_all_btn.set_label(t.upgrade_all);
        self.installed_btn.set_label(t.installed);
        self.available_btn.set_label(t.available);
        self.updates_btn.set_label(t.updates);
        self.orphans_btn.set_label(t.orphans);
        self.install_btn.set_label(t.install);
        self.remove_btn.set_label(t.remove);
        self.reinstall_btn.set_label(t.reinstall);
        self.prefs_btn.set_label(t.preferences);
        self.quit_btn.set_label(t.quit);
        self.remove_orphans_btn.set_label(t.remove_orphans);
    }
}

// ---------------------------------------------------------------------------
// Execução em segundo plano (a interface nunca bloqueia)
// ---------------------------------------------------------------------------

fn run_async<T, W, D>(ui: &Ui, lock: bool, work: W, done: D)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    D: FnOnce(T) + 'static,
{
    ui.begin(lock);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(work).await;
        ui.end(lock);
        if let Ok(value) = result {
            done(value);
        }
    });
}

// ---------------------------------------------------------------------------
// Desenho da lista
// ---------------------------------------------------------------------------

fn update_package_list(
    package_list: &ListBox,
    packages: &[Package],
    title: &str,
    empty_text: &str,
    installed: &HashSet<String>,
    installed_tip: &str,
) {
    while let Some(child) = package_list.first_child() {
        package_list.remove(&child);
    }

    let header_box = GtkBox::new(gtk4::Orientation::Horizontal, 0);
    header_box.set_margin_top(12);
    header_box.set_margin_bottom(8);
    header_box.set_margin_start(12);
    header_box.set_margin_end(12);
    let count_label = Label::new(Some(&format!("{} ({})", title, packages.len())));
    count_label.add_css_class("title-2");
    header_box.append(&count_label);

    let header_row = ListBoxRow::new();
    header_row.set_child(Some(&header_box));
    header_row.set_selectable(false);
    header_row.set_activatable(false);
    package_list.append(&header_row);

    let sep_row = ListBoxRow::new();
    sep_row.set_child(Some(&Separator::new(gtk4::Orientation::Horizontal)));
    sep_row.set_selectable(false);
    sep_row.set_activatable(false);
    package_list.append(&sep_row);

    if packages.is_empty() {
        let no_packages = Label::new(Some(empty_text));
        no_packages.set_halign(Align::Center);
        no_packages.set_margin_top(24);
        no_packages.set_margin_bottom(24);
        let empty_row = ListBoxRow::new();
        empty_row.set_child(Some(&no_packages));
        empty_row.set_selectable(false);
        empty_row.set_activatable(false);
        package_list.append(&empty_row);
        return;
    }

    for pkg in packages {
        let row_box = GtkBox::new(gtk4::Orientation::Horizontal, 12);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);

        // IMPORTANTE: o primeiro Label tem de ser o nome (usado na seleção)
        let name_label = Label::new(Some(&pkg.name));
        name_label.set_halign(Align::Start);
        name_label.set_width_request(200);
        name_label.set_xalign(0.0);
        name_label.add_css_class("heading");
        row_box.append(&name_label);

        name_label.set_hexpand(true);

        let ver_label = Label::new(Some(&pkg.version));
        ver_label.set_halign(Align::End);
        ver_label.set_width_request(220);
        ver_label.set_xalign(1.0);
        ver_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        ver_label.add_css_class("dim-label");
        row_box.append(&ver_label);

        if installed.contains(&pkg.name) {
            let badge = Label::new(Some("✔"));
            badge.add_css_class("success");
            badge.set_tooltip_text(Some(installed_tip));
            row_box.append(&badge);
        }

        let row = ListBoxRow::new();
        row.set_child(Some(&row_box));
        if !pkg.comment.is_empty() {
            row.set_tooltip_text(Some(&pkg.comment));
        }
        package_list.append(&row);
    }
}

/// NOTA: nunca chamar com um `st.borrow()` ativo — reconstruir a lista emite
/// `row-selected`, cujo handler faz `borrow_mut()`.
fn render(ui: &Ui, st: &Shared, packages: &[Package], title: &str, empty_text: &str) {
    let installed = st.borrow().installed_names.clone();
    let lang = st.borrow().lang;
    let t = get_translations(lang);
    update_package_list(&ui.list, packages, title, empty_text, &installed, t.installed);
}

// ---------------------------------------------------------------------------
// Pesquisa
// ---------------------------------------------------------------------------

fn search_cache(cache: &[Package], query: &str, limit: usize) -> Vec<Package> {
    let mut scored: Vec<(u8, &Package)> = cache
        .iter()
        .filter_map(|p| {
            let name = p.name.to_lowercase();
            let score = if name == query {
                0
            } else if name.starts_with(query) {
                1
            } else if name.contains(query) {
                2
            } else if p.comment.to_lowercase().contains(query) {
                3
            } else {
                return None;
            };
            Some((score, p))
        })
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
    scored.into_iter().take(limit).map(|(_, p)| p.clone()).collect()
}

fn schedule_search(ui: &Ui, st: &Shared) {
    let generation = {
        let mut s = st.borrow_mut();
        s.search_gen += 1;
        s.search_gen
    };
    let ui = ui.clone();
    let st = st.clone();
    glib::timeout_add_local_once(Duration::from_millis(SEARCH_DELAY_MS), move || {
        // Se o utilizador continuou a escrever, esta pesquisa já está obsoleta
        if st.borrow().search_gen != generation {
            return;
        }
        apply_search(&ui, &st);
    });
}

/// Ponto único de desenho: aplica o texto da pesquisa à vista atual.
fn apply_search(ui: &Ui, st: &Shared) {
    let t = get_translations(st.borrow().lang);
    let text = ui.search.text().trim().to_lowercase();
    let view = st.borrow().view;

    if view == View::Available {
        if text.is_empty() {
            render(ui, st, &[], t.title_available, t.available_hint);
            return;
        }
        let cache = st.borrow().available_cache.clone();
        match cache {
            Some(c) => {
                let results = search_cache(&c, &text, MAX_RESULTS);
                render(ui, st, &results, t.title_search, t.no_packages);
            }
            None => {
                let loading = st.borrow().loading_available;
                let msg = if loading { t.loading } else { t.no_packages };
                render(ui, st, &[], t.title_available, msg);
            }
        }
        return;
    }

    if text.is_empty() {
        let all = st.borrow().packages.clone();
        render(ui, st, &all, view.title(&t), t.no_packages);
    } else {
        let filtered: Vec<Package> = st
            .borrow()
            .packages
            .iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&text) || p.comment.to_lowercase().contains(&text)
            })
            .cloned()
            .collect();
        render(ui, st, &filtered, t.title_search, t.no_packages);
    }
}

// ---------------------------------------------------------------------------
// Vistas
// ---------------------------------------------------------------------------

/// O primeiro Label da linha é sempre o nome do pacote.
fn row_pkg_name(row: &ListBoxRow) -> Option<String> {
    row.child()
        .and_then(|c| c.downcast::<GtkBox>().ok())
        .and_then(|b| b.first_child())
        .and_then(|w| w.downcast::<Label>().ok())
        .map(|l| l.text().to_string())
}

fn update_actions(ui: &Ui, st: &Shared) {
    let (view, selected, installed) = {
        let s = st.borrow();
        (s.view, s.selected.clone(), s.installed_names.clone())
    };
    let (inst, rem, rein) = match selected {
        None => (false, false, false),
        Some(name) => {
            if view == View::Available {
                let is_installed = installed.contains(&name);
                (!is_installed, is_installed, is_installed)
            } else {
                (false, true, true)
            }
        }
    };
    ui.install_btn.set_sensitive(inst);
    ui.remove_btn.set_sensitive(rem);
    ui.reinstall_btn.set_sensitive(rein);
}

fn enter_available(ui: &Ui, st: &Shared) {
    let need_cache = st.borrow().available_cache.is_none();
    if need_cache {
        st.borrow_mut().loading_available = true;
    }
    let ui2 = ui.clone();
    let st2 = st.clone();
    // lock=false: pode continuar a escrever enquanto o catálogo carrega
    run_async(
        ui,
        false,
        move || {
            let cache = if need_cache {
                Some(pkg::list_available_packages())
            } else {
                None
            };
            (cache, pkg::list_installed_names())
        },
        move |(cache, names)| {
            {
                let mut s = st2.borrow_mut();
                s.installed_names = Rc::new(names);
                if let Some(c) = cache {
                    s.loading_available = false;
                    if !c.is_empty() {
                        s.available_cache = Some(Rc::new(c));
                    }
                }
            }
            let still_available = st2.borrow().view == View::Available;
            if still_available {
                apply_search(&ui2, &st2);
                update_actions(&ui2, &st2);
            }
        },
    );
}

fn load_view(ui: &Ui, st: &Shared, view: View, clear_search: bool) {
    if clear_search && !ui.search.text().is_empty() {
        ui.suppress_search.set(true);
        ui.search.set_text("");
        ui.suppress_search.set(false);
    }
    {
        let mut s = st.borrow_mut();
        s.view = view;
        s.selected = None;
        if view != View::Available {
            s.packages.clear();
        }
    }
    update_actions(ui, st);
    ui.orphans_bar.set_visible(view == View::Orphans);
    ui.remove_orphans_btn.set_sensitive(false);

    if view == View::Available {
        apply_search(ui, st);
        ui.search.grab_focus();
        enter_available(ui, st);
        return;
    }

    // Mostra "A carregar..." enquanto o pkg trabalha em segundo plano
    let t = get_translations(st.borrow().lang);
    render(ui, st, &[], view.title(&t), t.loading);

    let ui2 = ui.clone();
    let st2 = st.clone();
    run_async(
        ui,
        true,
        move || match view {
            View::Installed => pkg::list_installed_packages(),
            View::Updates => pkg::list_upgrades(),
            View::Orphans => pkg::list_orphan_packages(),
            View::Available => Vec::new(),
        },
        move |pkgs| {
            let same_view = st2.borrow().view == view;
            if !same_view {
                return;
            }
            {
                let mut s = st2.borrow_mut();
                if view == View::Installed {
                    s.installed_names = Rc::new(pkgs.iter().map(|p| p.name.clone()).collect());
                }
                s.packages = pkgs;
            }
            apply_search(&ui2, &st2);

            if view == View::Orphans {
                let has_orphans = !st2.borrow().packages.is_empty();
                ui2.remove_orphans_btn.set_sensitive(has_orphans);
            }
        },
    );
}

fn reload_current(ui: &Ui, st: &Shared) {
    let view = st.borrow().view;
    if view == View::Available {
        enter_available(ui, st);
    } else {
        load_view(ui, st, view, false);
    }
}

// ---------------------------------------------------------------------------
// Diálogos e ações
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Kind {
    Question,
    Warning,
    Error,
}

fn esc(s: &str) -> String {
    glib::markup_escape_text(s).to_string()
}

/// Diálogo uniforme: ícone grande, título, texto (markup), lista opcional de
/// pacotes e botões largos. `confirm = None` => só um botão "OK".
fn show_dialog<F>(
    ui: &Ui,
    kind: Kind,
    title: &str,
    markup: &str,
    items: &[String],
    confirm: Option<(&str, bool)>, // (texto do botão, é destrutivo?)
    cancel_label: &str,
    on_confirm: F,
) where
    F: Fn() + 'static,
{
    let dialog = gtk4::Window::builder()
        .transient_for(&ui.window)
        .modal(true)
        .resizable(false)
        .title(title)
        .default_width(480)
        .build();

    let root = GtkBox::new(gtk4::Orientation::Vertical, 0);

    let content = GtkBox::new(gtk4::Orientation::Vertical, 14);
    content.set_margin_top(28);
    content.set_margin_bottom(28);
    content.set_margin_start(36);
    content.set_margin_end(36);

    let (icon_name, icon_class) = match kind {
        Kind::Question => ("dialog-question-symbolic", "accent"),
        Kind::Warning => ("dialog-warning-symbolic", "warning"),
        Kind::Error => ("dialog-error-symbolic", "error"),
    };
    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(64);
    icon.add_css_class(icon_class);
    icon.set_margin_bottom(6);
    content.append(&icon);

    let title_label = Label::new(Some(title));
    title_label.add_css_class("title-2");
    title_label.set_wrap(true);
    title_label.set_justify(gtk4::Justification::Center);
    content.append(&title_label);

    let body = Label::new(None);
    body.set_markup(markup);
    body.set_wrap(true);
    body.set_justify(gtk4::Justification::Center);
    body.set_max_width_chars(46);
    content.append(&body);

    // Lista rolável de pacotes (ex.: órfãos a remover)
    if !items.is_empty() {
        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.add_css_class("boxed-list");
        for name in items {
            let row_box = GtkBox::new(gtk4::Orientation::Horizontal, 10);
            row_box.set_margin_top(8);
            row_box.set_margin_bottom(8);
            row_box.set_margin_start(12);
            row_box.set_margin_end(12);
            row_box.append(&Image::from_icon_name("package-x-generic-symbolic"));
            let l = Label::new(Some(name));
            l.set_xalign(0.0);
            l.set_hexpand(true);
            l.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            row_box.append(&l);
            let row = ListBoxRow::new();
            row.set_child(Some(&row_box));
            list.append(&row);
        }
        let scroller = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(220)
            .margin_top(6)
            .build();
        scroller.set_child(Some(&list));
        content.append(&scroller);
    }

    // Botões grandes, com espaço entre eles
    let buttons = GtkBox::new(gtk4::Orientation::Horizontal, 12);
    buttons.set_homogeneous(true);
    buttons.set_margin_top(14);

    match confirm {
        Some((label, destructive)) => {
            let cancel_btn = Button::with_label(cancel_label);
            cancel_btn.add_css_class("pill");
            let d = dialog.clone();
            cancel_btn.connect_clicked(move |_| d.close());
            buttons.append(&cancel_btn);

            let ok_btn = Button::with_label(label);
            ok_btn.add_css_class("pill");
            ok_btn.add_css_class(if destructive {
                "destructive-action"
            } else {
                "suggested-action"
            });
            let on_confirm = Rc::new(on_confirm);
            let d = dialog.clone();
            ok_btn.connect_clicked(move |_| {
                d.close();
                on_confirm();
            });
            buttons.append(&ok_btn);
            if !destructive {
                dialog.set_default_widget(Some(&ok_btn));
            }
        }
        None => {
            buttons.set_homogeneous(false);
            buttons.set_halign(Align::Center);
            let ok_btn = Button::with_label("OK");
            ok_btn.add_css_class("pill");
            ok_btn.add_css_class("suggested-action");
            ok_btn.set_width_request(180);
            let d = dialog.clone();
            ok_btn.connect_clicked(move |_| d.close());
            buttons.append(&ok_btn);
            dialog.set_default_widget(Some(&ok_btn));
        }
    }
    content.append(&buttons);
    root.append(&content);
    dialog.set_child(Some(&root));

    // Esc fecha o diálogo
    let key = gtk4::EventControllerKey::new();
    let d = dialog.clone();
    key.connect_key_pressed(move |_, keyval, _, _| {
        if keyval == gtk4::gdk::Key::Escape {
            d.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    dialog.add_controller(key);

    dialog.present();
}

fn show_message(ui: &Ui, kind: Kind, title: &str, markup: &str) {
    show_dialog(ui, kind, title, markup, &[], None, "", || {});
}

fn start_action(ui: &Ui, st: &Shared, action: Action) {
    let name = match st.borrow().selected.clone() {
        Some(n) => n,
        None => return,
    };

    if matches!(action, Action::Remove) && pkg::is_critical_package(&name) {
        show_message(
            ui,
            Kind::Error,
            "Atenção de Segurança",
            &format!(
                "O pacote <b>{}</b> é crítico do sistema e não pode ser removido!",
                esc(&name)
            ),
        );
        return;
    }

    let t = get_translations(st.borrow().lang);
    let q = esc(&name);
    let (title, text, kind, label, destructive) = match action {
        Action::Install => (
            "Instalar Pacote",
            format!("Pretende instalar o pacote <b>{}</b>?", q),
            Kind::Question,
            t.install,
            false,
        ),
        Action::Remove => (
            "Remover Pacote",
            format!("Tem a certeza de que pretende remover o pacote <b>{}</b>?", q),
            Kind::Warning,
            t.remove,
            true,
        ),
        Action::Reinstall => (
            "Reinstalar Pacote",
            format!("Pretende reinstalar o pacote <b>{}</b>?", q),
            Kind::Question,
            t.reinstall,
            false,
        ),
    };

    let (ui_c, st_c) = (ui.clone(), st.clone());
    show_dialog(
        ui,
        kind,
        title,
        &text,
        &[],
        Some((label, destructive)),
        t.cancel,
        move || {
            let name = name.clone();
            let (ui2, st2) = (ui_c.clone(), st_c.clone());
            run_async(
                &ui_c,
                true,
                move || match action {
                    Action::Install => pkg::install_package(&name),
                    Action::Remove => pkg::remove_package(&name),
                    Action::Reinstall => pkg::reinstall_package(&name),
                },
                move |ok| {
                    if ok {
                        reload_current(&ui2, &st2);
                    } else {
                        show_message(
                            &ui2,
                            Kind::Error,
                            "Erro",
                            "A operação falhou ou foi cancelada.",
                        );
                    }
                },
            );
        },
    );
}

fn confirm_remove_orphans(ui: &Ui, st: &Shared) {
    let names: Vec<String> = st
        .borrow()
        .packages
        .iter()
        .map(|p| p.name.clone())
        .filter(|n| !pkg::is_critical_package(n))
        .collect();
    if names.is_empty() {
        return;
    }

    let t = get_translations(st.borrow().lang);
    let text = format!(
        "Vão ser removidos <b>{}</b> pacote(s) órfão(s): instalados como dependência e que já nenhum outro pacote utiliza.",
        names.len()
    );

    let (ui_c, st_c) = (ui.clone(), st.clone());
    let to_remove = names.clone();
    show_dialog(
        ui,
        Kind::Warning,
        "Remover Órfãos",
        &text,
        &names,
        Some((t.remove, true)),
        t.cancel,
        move || {
            let names = to_remove.clone();
            let (ui2, st2) = (ui_c.clone(), st_c.clone());
            run_async(
                &ui_c,
                true,
                move || pkg::remove_packages(&names),
                move |ok| {
                    if ok {
                        reload_current(&ui2, &st2);
                    } else {
                        show_message(
                            &ui2,
                            Kind::Error,
                            "Erro",
                            "A operação falhou ou foi cancelada.",
                        );
                    }
                },
            );
        },
    );
}

fn heading(text: &str) -> Label {
    let l = Label::new(Some(text));
    l.add_css_class("heading");
    l.set_xalign(0.0);
    l.set_margin_top(6);
    l
}

fn add_escape_close(dialog: &gtk4::Window) {
    let key = gtk4::EventControllerKey::new();
    let d = dialog.clone();
    key.connect_key_pressed(move |_, keyval, _, _| {
        if keyval == gtk4::gdk::Key::Escape {
            d.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    dialog.add_controller(key);
}

fn show_prefs(ui: &Ui, st: &Shared) {
    let (lang, theme) = {
        let s = st.borrow();
        (s.lang, s.theme)
    };
    let t = get_translations(lang);

    let win = gtk4::Window::builder()
        .transient_for(&ui.window)
        .modal(true)
        .resizable(false)
        .title(t.prefs_title)
        .default_width(520)
        .build();

    let root = GtkBox::new(gtk4::Orientation::Vertical, 12);
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_margin_start(28);
    root.set_margin_end(28);

    // ---- Aparência: três cartões Sistema / Claro / Escuro
    root.append(&heading(t.prefs_appearance));
    let hint = Label::new(Some(t.prefs_appearance_hint));
    hint.add_css_class("dim-label");
    hint.set_xalign(0.0);
    root.append(&hint);

    let theme_box = GtkBox::new(gtk4::Orientation::Horizontal, 12);
    theme_box.set_homogeneous(true);
    theme_box.set_margin_top(6);
    theme_box.set_margin_bottom(10);

    let mut leader: Option<ToggleButton> = None;
    for (th, icon, label) in [
        (Theme::System, "preferences-desktop-theme-symbolic", t.theme_system),
        (Theme::Light, "weather-clear-symbolic", t.theme_light),
        (Theme::Dark, "weather-clear-night-symbolic", t.theme_dark),
    ] {
        let inner = GtkBox::new(gtk4::Orientation::Vertical, 8);
        inner.set_margin_top(14);
        inner.set_margin_bottom(14);
        let img = Image::from_icon_name(icon);
        img.set_pixel_size(32);
        inner.append(&img);
        inner.append(&Label::new(Some(label)));

        let btn = ToggleButton::new();
        btn.set_child(Some(&inner));
        match leader.clone() {
            Some(l) => btn.set_group(Some(&l)),
            None => leader = Some(btn.clone()),
        }
        btn.set_active(th == theme);

        let st_c = st.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                st_c.borrow_mut().theme = th;
                apply_theme(th);
                persist_settings(&st_c);
            }
        });
        theme_box.append(&btn);
    }
    root.append(&theme_box);

    // ---- Idioma
    root.append(&heading(t.prefs_language));
    let dd = DropDown::from_strings(&["English", "Português"]);
    dd.set_selected(if lang == Language::English { 0 } else { 1 });
    let (ui_c, st_c) = (ui.clone(), st.clone());
    dd.connect_selected_notify(move |d| {
        let new_lang = if d.selected() == 0 {
            Language::English
        } else {
            Language::Portuguese
        };
        st_c.borrow_mut().lang = new_lang;
        ui_c.apply_language(&get_translations(new_lang));
        apply_search(&ui_c, &st_c);
        persist_settings(&st_c);
    });
    root.append(&dd);

    let close_btn = Button::with_label(t.close);
    close_btn.add_css_class("pill");
    close_btn.set_halign(Align::End);
    close_btn.set_margin_top(14);
    close_btn.set_width_request(160);
    let w = win.clone();
    close_btn.connect_clicked(move |_| w.close());
    root.append(&close_btn);

    win.set_child(Some(&root));
    add_escape_close(&win);
    win.present();
}

// ---------------------------------------------------------------------------
// Detalhes do pacote
// ---------------------------------------------------------------------------

fn kv_row(title: &str, value: &impl IsA<gtk4::Widget>) -> ListBoxRow {
    let b = GtkBox::new(gtk4::Orientation::Horizontal, 12);
    b.set_margin_top(10);
    b.set_margin_bottom(10);
    b.set_margin_start(12);
    b.set_margin_end(12);
    let k = Label::new(Some(title));
    k.add_css_class("dim-label");
    k.set_xalign(0.0);
    k.set_valign(Align::Start);
    k.set_width_request(130);
    b.append(&k);
    b.append(value);
    let row = ListBoxRow::new();
    row.set_child(Some(&b));
    row.set_activatable(false);
    row.set_selectable(false);
    row
}

fn text_value(value: &str) -> Label {
    let l = Label::new(Some(value));
    l.set_wrap(true);
    l.set_xalign(0.0);
    l.set_hexpand(true);
    l.set_selectable(true);
    l
}

fn deps_expander(
    ui: &Ui,
    st: &Shared,
    dialog: &gtk4::Window,
    title: &str,
    items: &[(String, String)],
    none_text: &str,
    expanded: bool,
) -> Expander {
    let exp = Expander::new(None);
    let head = heading(&format!("{} ({})", title, items.len()));
    head.set_margin_top(0);
    exp.set_label_widget(Some(&head));

    if items.is_empty() {
        let l = Label::new(Some(none_text));
        l.add_css_class("dim-label");
        l.set_xalign(0.0);
        l.set_margin_top(6);
        exp.set_child(Some(&l));
        return exp;
    }

    let list = ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    for (name, version) in items {
        let b = GtkBox::new(gtk4::Orientation::Horizontal, 10);
        b.set_margin_top(8);
        b.set_margin_bottom(8);
        b.set_margin_start(12);
        b.set_margin_end(12);
        b.append(&Image::from_icon_name("package-x-generic-symbolic"));
        let n = Label::new(Some(name));
        n.set_xalign(0.0);
        n.set_hexpand(true);
        n.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        b.append(&n);
        let v = Label::new(Some(version));
        v.add_css_class("dim-label");
        b.append(&v);
        let row = ListBoxRow::new();
        row.set_child(Some(&b));
        list.append(&row);
    }

    // Clicar numa dependência abre o detalhe dela
    let names: Vec<String> = items.iter().map(|(n, _)| n.clone()).collect();
    let (ui_c, st_c, dlg) = (ui.clone(), st.clone(), dialog.clone());
    list.connect_row_activated(move |_, row| {
        if let Some(n) = names.get(row.index() as usize) {
            dlg.close();
            show_details(&ui_c, &st_c, n.clone());
        }
    });

    let frame = Frame::new(None);
    frame.set_child(Some(&list));
    frame.set_margin_top(6);
    exp.set_child(Some(&frame));
    exp.set_expanded(expanded);
    exp
}

fn show_details(ui: &Ui, st: &Shared, name: String) {
    let installed = st.borrow().installed_names.contains(&name);
    let (ui2, st2) = (ui.clone(), st.clone());
    run_async(
        ui,
        true,
        move || pkg::package_details(&name, installed),
        move |d| build_details_dialog(&ui2, &st2, d),
    );
}

fn build_details_dialog(ui: &Ui, st: &Shared, d: PackageDetails) {
    let t = get_translations(st.borrow().lang);

    let dialog = gtk4::Window::builder()
        .transient_for(&ui.window)
        .modal(true)
        .title(d.name.as_str())
        .default_width(580)
        .default_height(700)
        .build();
    add_escape_close(&dialog);

    let root = GtkBox::new(gtk4::Orientation::Vertical, 0);

    let content = GtkBox::new(gtk4::Orientation::Vertical, 18);
    content.set_margin_top(24);
    content.set_margin_bottom(18);
    content.set_margin_start(32);
    content.set_margin_end(32);

    // ---- Cabeçalho do pacote
    let hero = GtkBox::new(gtk4::Orientation::Vertical, 6);
    hero.set_halign(Align::Center);
    let icon = Image::from_icon_name("package-x-generic-symbolic");
    icon.set_pixel_size(64);
    icon.add_css_class("accent");
    hero.append(&icon);

    let name_label = Label::new(Some(&d.name));
    name_label.add_css_class("title-1");
    name_label.set_wrap(true);
    name_label.set_selectable(true);
    name_label.set_justify(gtk4::Justification::Center);
    hero.append(&name_label);

    if !d.version.is_empty() {
        let v = Label::new(Some(&d.version));
        v.add_css_class("dim-label");
        hero.append(&v);
    }

    let state = Label::new(Some(&if d.installed {
        format!("✔ {}", t.status_installed)
    } else {
        t.status_not_installed.to_string()
    }));
    state.add_css_class(if d.installed { "success" } else { "dim-label" });
    hero.append(&state);

    if !d.comment.is_empty() {
        let c = Label::new(Some(&d.comment));
        c.set_wrap(true);
        c.set_justify(gtk4::Justification::Center);
        c.set_margin_top(6);
        hero.append(&c);
    }
    content.append(&hero);

    // ---- Especificações
    let info = ListBox::new();
    info.set_selection_mode(gtk4::SelectionMode::None);
    let mut rows = 0;
    let mut add = |title: &str, value: &str| {
        if !value.is_empty() {
            info.append(&kv_row(title, &text_value(value)));
            rows += 1;
        }
    };
    add(t.origin, &d.origin);
    add(t.categories, &d.categories.join(", "));
    add(t.maintainer, &d.maintainer);
    add(t.license, &d.licenses.join(", "));
    add(t.architecture, &d.arch);
    add(t.size, &d.size);
    if !d.www.is_empty() && d.www != "UNKNOWN" {
        let link = LinkButton::with_label(&d.www, &d.www);
        link.set_halign(Align::Start);
        link.set_hexpand(true);
        info.append(&kv_row(t.website, &link));
        rows += 1;
    }
    if rows > 0 {
        let frame = Frame::new(None);
        frame.set_child(Some(&info));
        content.append(&frame);
    }

    // ---- Dependências e dependentes (clicáveis)
    content.append(&deps_expander(ui, st, &dialog, t.dependencies, &d.depends, t.none, true));
    content.append(&deps_expander(ui, st, &dialog, t.required_by, &d.required_by, t.none, false));

    // ---- Descrição completa
    if !d.description.is_empty() && d.description != d.comment {
        content.append(&heading(t.description));
        let desc = text_value(&d.description);
        content.append(&desc);
    }

    let scroller = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .build();
    scroller.set_child(Some(&content));
    root.append(&scroller);

    // ---- Barra de botões
    let bar = GtkBox::new(gtk4::Orientation::Horizontal, 12);
    bar.set_homogeneous(true);
    bar.set_margin_top(12);
    bar.set_margin_bottom(24);
    bar.set_margin_start(32);
    bar.set_margin_end(32);

    let close_btn = Button::with_label(t.close);
    close_btn.add_css_class("pill");
    let dlg = dialog.clone();
    close_btn.connect_clicked(move |_| dlg.close());
    bar.append(&close_btn);

    let actions: Vec<(Action, &str, &str)> = if d.installed {
        vec![
            (Action::Reinstall, t.reinstall, ""),
            (Action::Remove, t.remove, "destructive-action"),
        ]
    } else {
        vec![(Action::Install, t.install, "suggested-action")]
    };
    for (action, label, css) in actions {
        let btn = Button::with_label(label);
        btn.add_css_class("pill");
        if !css.is_empty() {
            btn.add_css_class(css);
        }
        let (ui_c, st_c, dlg, name) = (ui.clone(), st.clone(), dialog.clone(), d.name.clone());
        btn.connect_clicked(move |_| {
            dlg.close();
            st_c.borrow_mut().selected = Some(name.clone());
            start_action(&ui_c, &st_c, action);
        });
        bar.append(&btn);
    }
    root.append(&bar);

    dialog.set_child(Some(&root));
    dialog.present();
}

// ---------------------------------------------------------------------------
// Construção da janela
// ---------------------------------------------------------------------------

/// Cores explícitas para os botões de ação: garante texto legível mesmo
/// quando o tema GTK do sistema sobrepõe o estilo do libadwaita.
fn install_css() {
    const CSS: &str = "
        .title-1 { font-size: 22pt; font-weight: 800; }
        .title-2 { font-size: 15pt; font-weight: 800; }
        .heading { font-weight: bold; }
        .dim-label { opacity: 0.6; }
        .success { color: #26a269; }
        .warning { color: #e5a50a; }
        .error { color: #c01c28; }
        .accent { color: #3584e4; }
        button.pill { border-radius: 9999px; padding: 8px 24px; }

        button.suggested-action,
        button.suggested-action label {
            background-image: none;
            background-color: #3584e4;
            color: #ffffff;
            font-weight: bold;
        }
        button.suggested-action label { background-color: transparent; }
        button.suggested-action:hover { background-color: #4a94ef; }

        button.destructive-action,
        button.destructive-action label {
            background-image: none;
            background-color: #c01c28;
            color: #ffffff;
            font-weight: bold;
        }
        button.destructive-action label { background-color: transparent; }
        button.destructive-action:hover { background-color: #e01b24; }

        button.suggested-action:disabled,
        button.destructive-action:disabled {
            background-color: #7a7a7a;
            color: #d8d8d8;
        }
        button.suggested-action:disabled label,
        button.destructive-action:disabled label {
            color: #d8d8d8;
        }
    ";
    if let Some(display) = gtk4::gdk::Display::default() {
        let provider = gtk4::CssProvider::new();
        provider.load_from_data(CSS);
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_USER,
        );
    }
}

fn build_ui(app: &Application) {
    install_css();
    let (lang, theme) = load_settings();
    apply_theme(theme);
    let tr = get_translations(lang);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("PkGUI - FreeBSD Software Manager")
        .default_width(1150)
        .default_height(750)
        .build();

    let main_box = GtkBox::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&main_box));

    // Barra superior
    let top_bar = GtkBox::new(gtk4::Orientation::Horizontal, 12);
    top_bar.set_margin_top(12);
    top_bar.set_margin_bottom(12);
    top_bar.set_margin_start(12);
    top_bar.set_margin_end(12);

    let search = Entry::builder()
        .placeholder_text(tr.search_placeholder)
        .primary_icon_name("edit-find-symbolic")
        .hexpand(true)
        .build();
    top_bar.append(&search);

    let spinner = Spinner::new();
    spinner.set_size_request(24, 24);
    top_bar.append(&spinner);

    let top_actions = GtkBox::new(gtk4::Orientation::Horizontal, 6);
    let refresh_btn = Button::with_label(tr.refresh);
    let update_all_btn = Button::with_label(tr.update_all);
    let upgrade_all_btn = Button::with_label(tr.upgrade_all);
    top_actions.append(&refresh_btn);
    top_actions.append(&update_all_btn);
    top_actions.append(&upgrade_all_btn);
    top_bar.append(&top_actions);
    main_box.append(&top_bar);

    // Conteúdo
    let content_box = GtkBox::new(gtk4::Orientation::Horizontal, 0);
    content_box.set_vexpand(true);
    main_box.append(&content_box);

    let sidebar = GtkBox::new(gtk4::Orientation::Vertical, 6);
    sidebar.set_margin_top(12);
    sidebar.set_margin_bottom(12);
    sidebar.set_margin_start(12);
    sidebar.set_margin_end(12);
    sidebar.set_width_request(180);
    content_box.append(&sidebar);

    let installed_btn = Button::with_label(tr.installed);
    let available_btn = Button::with_label(tr.available);
    let updates_btn = Button::with_label(tr.updates);
    let orphans_btn = Button::with_label(tr.orphans);
    sidebar.append(&installed_btn);
    sidebar.append(&available_btn);
    sidebar.append(&updates_btn);
    sidebar.append(&orphans_btn);

    let separator = Separator::new(gtk4::Orientation::Horizontal);
    separator.set_margin_top(12);
    separator.set_margin_bottom(12);
    sidebar.append(&separator);

    let install_btn = Button::with_label(tr.install);
    let remove_btn = Button::with_label(tr.remove);
    let reinstall_btn = Button::with_label(tr.reinstall);
    install_btn.set_sensitive(false);
    remove_btn.set_sensitive(false);
    reinstall_btn.set_sensitive(false);
    sidebar.append(&install_btn);
    sidebar.append(&remove_btn);
    sidebar.append(&reinstall_btn);

    let sep_bottom = Separator::new(gtk4::Orientation::Horizontal);
    sep_bottom.set_margin_top(12);
    sep_bottom.set_margin_bottom(12);
    sidebar.append(&sep_bottom);

    let prefs_btn = Button::with_label(tr.preferences);
    let quit_btn = Button::with_label(tr.quit);
    sidebar.append(&prefs_btn);
    sidebar.append(&quit_btn);

    let package_area = GtkBox::new(gtk4::Orientation::Vertical, 0);
    package_area.set_hexpand(true);
    package_area.set_vexpand(true);
    content_box.append(&package_area);

    // Barra visível apenas na vista de órfãos
    let orphans_bar = GtkBox::new(gtk4::Orientation::Horizontal, 6);
    orphans_bar.set_margin_top(8);
    orphans_bar.set_margin_bottom(4);
    orphans_bar.set_margin_start(12);
    orphans_bar.set_margin_end(12);
    let remove_orphans_btn = Button::with_label(tr.remove_orphans);
    remove_orphans_btn.add_css_class("destructive-action");
    remove_orphans_btn.set_hexpand(true);
    remove_orphans_btn.set_halign(Align::End);
    remove_orphans_btn.set_sensitive(false);
    orphans_bar.append(&remove_orphans_btn);
    orphans_bar.set_visible(false);
    package_area.append(&orphans_bar);

    let scrolled = ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);
    let list = ListBox::new();
    scrolled.set_child(Some(&list));
    package_area.append(&scrolled);

    let ui = Ui {
        window: window.clone(),
        search,
        spinner,
        top_actions,
        sidebar,
        list,
        orphans_bar,
        remove_orphans_btn,
        refresh_btn,
        update_all_btn,
        upgrade_all_btn,
        installed_btn,
        available_btn,
        updates_btn,
        orphans_btn,
        install_btn,
        remove_btn,
        reinstall_btn,
        prefs_btn,
        quit_btn,
        busy: Rc::new(Cell::new(0)),
        locks: Rc::new(Cell::new(0)),
        suppress_search: Rc::new(Cell::new(false)),
    };

    let st: Shared = Rc::new(RefCell::new(State {
        lang,
        theme,
        view: View::Installed,
        packages: Vec::new(),
        selected: None,
        installed_names: Rc::new(HashSet::new()),
        available_cache: None,
        loading_available: false,
        search_gen: 0,
    }));

    // Pesquisa em tempo real (debounce de 200 ms)
    {
        let (ui_c, st_c) = (ui.clone(), st.clone());
        ui.search.connect_changed(move |_| {
            if ui_c.suppress_search.get() {
                return;
            }
            schedule_search(&ui_c, &st_c);
        });
    }

    // Seleção de linha
    {
        let (ui_c, st_c) = (ui.clone(), st.clone());
        ui.list.connect_row_selected(move |_, row| {
            let name = row.and_then(row_pkg_name);
            st_c.borrow_mut().selected = name;
            update_actions(&ui_c, &st_c);
        });
    }

    // Clicar num pacote abre o diálogo com as especificações
    {
        let (ui_c, st_c) = (ui.clone(), st.clone());
        ui.list.connect_row_activated(move |_, row| {
            if let Some(name) = row_pkg_name(row) {
                st_c.borrow_mut().selected = Some(name.clone());
                update_actions(&ui_c, &st_c);
                show_details(&ui_c, &st_c, name);
            }
        });
    }

    // Navegação lateral
    for (btn, view) in [
        (&ui.installed_btn, View::Installed),
        (&ui.available_btn, View::Available),
        (&ui.updates_btn, View::Updates),
        (&ui.orphans_btn, View::Orphans),
    ] {
        let (ui_c, st_c) = (ui.clone(), st.clone());
        btn.connect_clicked(move |_| load_view(&ui_c, &st_c, view, true));
    }

    // Botão "Remover Todos os Órfãos" (só visível na vista de órfãos)
    {
        let (ui_c, st_c) = (ui.clone(), st.clone());
        ui.remove_orphans_btn
            .connect_clicked(move |_| confirm_remove_orphans(&ui_c, &st_c));
    }

    // Ações sobre o pacote selecionado
    for (btn, action) in [
        (&ui.install_btn, Action::Install),
        (&ui.remove_btn, Action::Remove),
        (&ui.reinstall_btn, Action::Reinstall),
    ] {
        let (ui_c, st_c) = (ui.clone(), st.clone());
        btn.connect_clicked(move |_| start_action(&ui_c, &st_c, action));
    }

    // Refresh = recarregar a vista atual
    {
        let (ui_c, st_c) = (ui.clone(), st.clone());
        ui.refresh_btn.connect_clicked(move |_| {
            st_c.borrow_mut().available_cache = None;
            reload_current(&ui_c, &st_c);
        });
    }

    // Sincronizar repositórios (pkg update)
    {
        let (ui_c, st_c) = (ui.clone(), st.clone());
        ui.update_all_btn.connect_clicked(move |_| {
            let (ui2, st2) = (ui_c.clone(), st_c.clone());
            run_async(&ui_c, true, pkg::refresh_repositories, move |_| {
                st2.borrow_mut().available_cache = None;
                reload_current(&ui2, &st2);
            });
        });
    }

    // Atualizar tudo (pkg upgrade -y)
    {
        let (ui_c, st_c) = (ui.clone(), st.clone());
        ui.upgrade_all_btn.connect_clicked(move |_| {
            let (ui2, st2) = (ui_c.clone(), st_c.clone());
            run_async(&ui_c, true, pkg::upgrade_all_packages, move |_| {
                reload_current(&ui2, &st2);
            });
        });
    }

    {
        let (ui_c, st_c) = (ui.clone(), st.clone());
        ui.prefs_btn.connect_clicked(move |_| show_prefs(&ui_c, &st_c));
    }
    {
        let win = window.clone();
        ui.quit_btn.connect_clicked(move |_| win.close());
    }

    load_view(&ui, &st, View::Installed, true);
    window.present();
}

pub fn run() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("com.github.Olen1962.pkgui")
        .build();
    app.connect_activate(build_ui);
    app.run()
}