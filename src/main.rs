use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Button, Entry, Label, ListBox, ScrolledWindow};
use libadwaita::{HeaderBar, WindowTitle};
use std::process::Command;

struct Package {
    name: String,
    version: String,
}

fn list_installed_packages() -> Vec<Package> {
    let output = Command::new("pkg")
        .arg("query")
        .arg("%n-%v")
        .output();

    match output {
        Ok(result) if result.status.success() => {
            String::from_utf8_lossy(&result.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    let parts: Vec<&str> = line.trim().split('-').collect();
                    let name = parts.get(0).unwrap_or(&line).to_string();
                    let version = parts.get(1).unwrap_or(&"").to_string();
                    Package { name, version }
                })
                .collect()
        }
        _ => vec![],
    }
}

fn list_orphan_packages() -> Vec<Package> {
    let output = Command::new("pkg")
        .arg("query")
        .arg("%n-%v")
        .arg("-a")
        .output();

    match output {
        Ok(result) if result.status.success() => {
            String::from_utf8_lossy(&result.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    let parts: Vec<&str> = line.trim().split('-').collect();
                    let name = parts.get(0).unwrap_or(&line).to_string();
                    let version = parts.get(1).unwrap_or(&"").to_string();
                    Package { name, version }
                })
                .collect()
        }
        _ => vec![],
    }
}

fn list_dependencies() -> Vec<Package> {
    let output = Command::new("pkg")
        .arg("query")
        .arg("%n-%v")
        .arg("-d")
        .output();

    match output {
        Ok(result) if result.status.success() => {
            String::from_utf8_lossy(&result.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    let parts: Vec<&str> = line.trim().split('-').collect();
                    let name = parts.get(0).unwrap_or(&line).to_string();
                    let version = parts.get(1).unwrap_or(&"").to_string();
                    Package { name, version }
                })
                .collect()
        }
        _ => vec![],
    }
}

fn create_package_row(package: &Package) -> Box {
    let row_box = Box::new(gtk4::Orientation::Horizontal, 12);
    row_box.set_margin_top(8);
    row_box.set_margin_bottom(8);
    row_box.set_margin_start(12);
    row_box.set_margin_end(12);

    let name_label = Label::new(Some(&package.name));
    name_label.set_halign(gtk4::Align::Start);
    name_label.set_hexpand(true);
    name_label.add_css_class("title-3");
    row_box.append(&name_label);

    let version_label = Label::new(Some(&package.version));
    version_label.set_halign(gtk4::Align::End);
    row_box.append(&version_label);

    row_box
}

fn update_package_list(package_list: &ListBox, packages: Vec<Package>) {
    while let Some(child) = package_list.first_child() {
        package_list.remove(&child);
    }

    for pkg in packages {
        let row = create_package_row(&pkg);
        package_list.append(&row);
    }

    if package_list.first_child().is_none() {
        let no_packages = Label::new(Some("No packages found."));
        no_packages.set_halign(gtk4::Align::Center);
        no_packages.set_margin_top(24);
        no_packages.set_margin_bottom(24);
        package_list.append(&no_packages);
    }
}

fn create_preferences_dialog(parent: &ApplicationWindow) {
    let dialog = gtk4::Window::builder()
        .title("Preferences")
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(300)
        .build();

    let main_box = Box::new(gtk4::Orientation::Vertical, 24);
    main_box.set_margin_top(24);
    main_box.set_margin_bottom(24);
    main_box.set_margin_start(24);
    main_box.set_margin_end(24);
    dialog.set_child(Some(&main_box));

    let theme_label = Label::new(Some("Theme:"));
    theme_label.set_halign(gtk4::Align::Start);
    main_box.append(&theme_label);

    let theme_box = Box::new(gtk4::Orientation::Horizontal, 12);
    let light_btn = Button::with_label("Light");
    let dark_btn = Button::with_label("Dark");
    let system_btn = Button::with_label("System");
    theme_box.append(&light_btn);
    theme_box.append(&dark_btn);
    theme_box.append(&system_btn);
    main_box.append(&theme_box);

    let lang_label = Label::new(Some("Language:"));
    lang_label.set_halign(gtk4::Align::Start);
    main_box.append(&lang_label);

    let lang_combo = gtk4::DropDown::from_strings(&["System", "English", "Portuguese"]);
    main_box.append(&lang_combo);

    let ok_btn = Button::with_label("OK");
    ok_btn.set_halign(gtk4::Align::End);
    let dialog_clone = dialog.clone();
    ok_btn.connect_clicked(move |_| {
        dialog_clone.close();
    });
    main_box.append(&ok_btn);

    dialog.show();
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("com.github.Olen1962.pkgui")
        .build();

    app.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("PkGUI - BSD Package Manager")
            .default_width(1000)
            .default_height(700)
            .build();

        let header = HeaderBar::builder()
            .title_widget(&WindowTitle::new("PkGUI", "BSD Package Manager"))
            .show_end_title_buttons(true)
            .build();
        window.set_titlebar(Some(&header));

        let main_box = Box::new(gtk4::Orientation::Vertical, 0);
        window.set_child(Some(&main_box));

        // ===== TOP BAR =====
        let top_bar = Box::new(gtk4::Orientation::Horizontal, 12);
        top_bar.set_margin_top(12);
        top_bar.set_margin_bottom(12);
        top_bar.set_margin_start(12);
        top_bar.set_margin_end(12);

        let search_entry = Entry::builder()
            .placeholder_text("Software search")
            .hexpand(true)
            .build();
        top_bar.append(&search_entry);

        let refresh_btn = Button::with_label("Refresh");
        top_bar.append(&refresh_btn);

        main_box.append(&top_bar);

        // ===== MAIN CONTENT (Horizontal: Sidebar + Package List) =====
        let content_box = Box::new(gtk4::Orientation::Horizontal, 0);
        main_box.append(&content_box);

        // ===== SIDEBAR =====
        let sidebar = Box::new(gtk4::Orientation::Vertical, 6);
        sidebar.set_margin_top(12);
        sidebar.set_margin_bottom(12);
        sidebar.set_margin_start(12);
        sidebar.set_margin_end(12);
        sidebar.set_width_request(180);
        content_box.append(&sidebar);

        let install_btn = Button::with_label("Install");
        let remove_btn = Button::with_label("Remove");
        let orphans_btn = Button::with_label("Orphans");
        let installed_btn = Button::with_label("Installed");
        let deps_btn = Button::with_label("Dependencies");
        let prefs_btn = Button::with_label("Preferences");
        let quit_btn = Button::with_label("Quit");

        let window_clone = window.clone();
        quit_btn.connect_clicked(move |_| {
            window_clone.close();
        });

        let window_clone2 = window.clone();
        prefs_btn.connect_clicked(move |_| {
            create_preferences_dialog(&window_clone2);
        });

        sidebar.append(&install_btn);
        sidebar.append(&remove_btn);
        sidebar.append(&orphans_btn);
        sidebar.append(&installed_btn);
        sidebar.append(&deps_btn);
        sidebar.append(&prefs_btn);
        sidebar.append(&quit_btn);

        // ===== PACKAGE LIST =====
        let package_area = Box::new(gtk4::Orientation::Vertical, 0);
        package_area.set_hexpand(true);
        content_box.append(&package_area);

        let scrolled = ScrolledWindow::new();
        let package_list = ListBox::new();
        package_list.set_selection_mode(gtk4::SelectionMode::None);
        scrolled.set_child(Some(&package_list));
        package_area.append(&scrolled);

        // Clone for signals
        let package_list_clone = package_list.clone();
        let package_list_clone2 = package_list.clone();
        let package_list_clone3 = package_list.clone();
        let package_list_clone4 = package_list.clone();

        refresh_btn.connect_clicked(move |_| {
            let packages = list_installed_packages();
            update_package_list(&package_list_clone, packages);
        });

        installed_btn.connect_clicked(move |_| {
            let packages = list_installed_packages();
            update_package_list(&package_list_clone2, packages);
        });

        orphans_btn.connect_clicked(move |_| {
            let packages = list_orphan_packages();
            update_package_list(&package_list_clone3, packages);
        });

        deps_btn.connect_clicked(move |_| {
            let packages = list_dependencies();
            update_package_list(&package_list_clone4, packages);
        });

        // Initial load
        let packages = list_installed_packages();
        update_package_list(&package_list, packages);

        window.show();
    });

    app.run()
}
