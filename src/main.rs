use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Button, Entry, Grid, Label, ScrolledWindow, Separator, Align};
use libadwaita::{HeaderBar, WindowTitle};
use std::process::Command;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Package {
    name: String,
    version: String,
    origin: String,
    comment: String,
}

fn list_installed_packages() -> Vec<Package> {
    let output = Command::new("pkg")
        .arg("query")
        .arg("%n\t%v\t%o\t%c")
        .output();

    match output {
        Ok(result) if result.status.success() => {
            String::from_utf8_lossy(&result.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    let parts: Vec<&str> = line.split('\t').collect();
                    Package {
                        name: parts.get(0).unwrap_or(&"").to_string(),
                        version: parts.get(1).unwrap_or(&"").to_string(),
                        origin: parts.get(2).unwrap_or(&"").to_string(),
                        comment: parts.get(3).unwrap_or(&"").to_string(),
                    }
                })
                .collect()
        }
        Ok(result) => {
            eprintln!("pkg query stderr: {}", String::from_utf8_lossy(&result.stderr));
            vec![]
        }
        Err(e) => {
            eprintln!("pkg query error: {}", e);
            vec![]
        }
    }
}

fn list_orphan_packages() -> Vec<Package> {
    let output = Command::new("pkg")
        .arg("autoremove")
        .arg("-n")
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let mut names = Vec::new();
            let mut in_list = false;

            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Installed packages to be REMOVED:") {
                    in_list = true;
                    continue;
                }
                if in_list {
                    if trimmed.is_empty() {
                        in_list = false;
                        continue;
                    }
                    let clean = trimmed.trim_start_matches('\t').trim();
                    if !clean.is_empty() {
                        let parts: Vec<&str> = clean.splitn(2, ':').collect();
                        if !parts.is_empty() {
                            names.push(parts[0].trim().to_string());
                        }
                    }
                }
            }

            if names.is_empty() {
                return vec![];
            }

            let mut packages = Vec::new();
            for name in names {
                let query_out = Command::new("pkg")
                    .arg("query")
                    .arg("-e")
                    .arg(&format!("%n = '{}'", name))
                    .arg("%n\t%v\t%o\t%c")
                    .output();

                if let Ok(res) = query_out {
                    if res.status.success() {
                        for line in String::from_utf8_lossy(&res.stdout).lines() {
                            let parts: Vec<&str> = line.split('\t').collect();
                            if !parts.is_empty() {
                                packages.push(Package {
                                    name: parts.get(0).unwrap_or(&"").to_string(),
                                    version: parts.get(1).unwrap_or(&"").to_string(),
                                    origin: parts.get(2).unwrap_or(&"").to_string(),
                                    comment: parts.get(3).unwrap_or(&"").to_string(),
                                });
                            }
                        }
                    }
                }
            }
            packages
        }
        Ok(result) => {
            eprintln!("pkg autoremove stderr: {}", String::from_utf8_lossy(&result.stderr));
            vec![]
        }
        Err(e) => {
            eprintln!("pkg autoremove error: {}", e);
            vec![]
        }
    }
}

fn list_dependencies() -> Vec<Package> {
    let output = Command::new("pkg")
        .arg("query")
        .arg("-e")
        .arg("%a = 1")
        .arg("%n\t%v\t%o\t%c")
        .output();

    match output {
        Ok(result) if result.status.success() => {
            String::from_utf8_lossy(&result.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    let parts: Vec<&str> = line.split('\t').collect();
                    Package {
                        name: parts.get(0).unwrap_or(&"").to_string(),
                        version: parts.get(1).unwrap_or(&"").to_string(),
                        origin: parts.get(2).unwrap_or(&"").to_string(),
                        comment: parts.get(3).unwrap_or(&"").to_string(),
                    }
                })
                .collect()
        }
        Ok(result) => {
            eprintln!("pkg query stderr: {}", String::from_utf8_lossy(&result.stderr));
            vec![]
        }
        Err(e) => {
            eprintln!("pkg query error: {}", e);
            vec![]
        }
    }
}

fn filter_packages(packages: &[Package], search_text: &str) -> Vec<Package> {
    if search_text.is_empty() {
        return packages.to_vec();
    }
    
    let search_lower = search_text.to_lowercase();
    packages
        .iter()
        .filter(|pkg| {
            pkg.name.to_lowercase().contains(&search_lower) ||
            pkg.version.to_lowercase().contains(&search_lower) ||
            pkg.origin.to_lowercase().contains(&search_lower) ||
            pkg.comment.to_lowercase().contains(&search_lower)
        })
        .cloned()
        .collect()
}

fn update_package_list(scrolled: &ScrolledWindow, packages: Vec<Package>, title: &str) {
    let outer_box = Box::new(gtk4::Orientation::Vertical, 0);
    outer_box.set_margin_top(16);
    outer_box.set_margin_bottom(16);
    outer_box.set_margin_start(16);
    outer_box.set_margin_end(16);

    let count_label = Label::new(Some(&format!("{} ({} packages)", title, packages.len())));
    count_label.add_css_class("title-2");
    count_label.set_halign(Align::Start);
    count_label.set_margin_bottom(12);
    outer_box.append(&count_label);

    if packages.is_empty() {
        let no_packages = Label::new(Some("No packages found."));
        no_packages.set_halign(Align::Center);
        no_packages.set_margin_top(24);
        no_packages.set_margin_bottom(24);
        outer_box.append(&no_packages);
        scrolled.set_child(Some(&outer_box));
        return;
    }

    let grid = Grid::new();
    grid.set_column_spacing(24);
    grid.set_row_spacing(8);

    let headers = ["Name", "Version", "Origin", "Description"];
    for (col, text) in headers.iter().enumerate() {
        let label = Label::new(Some(text));
        label.set_halign(Align::Start);
        label.set_xalign(0.0);
        label.add_css_class("title-4");
        if col == 3 {
            label.set_hexpand(true);
        }
        grid.attach(&label, col as i32, 0, 1, 1);
    }

    let sep = Separator::new(gtk4::Orientation::Horizontal);
    sep.set_margin_top(4);
    sep.set_margin_bottom(8);
    grid.attach(&sep, 0, 1, 4, 1);

    for (i, pkg) in packages.iter().enumerate() {
        let row_idx = (i + 2) as i32;

        let name_label = Label::new(Some(&pkg.name));
        name_label.set_halign(Align::Start);
        name_label.set_xalign(0.0);
        grid.attach(&name_label, 0, row_idx, 1, 1);

        let version_label = Label::new(Some(&pkg.version));
        version_label.set_halign(Align::Start);
        version_label.set_xalign(0.0);
        grid.attach(&version_label, 1, row_idx, 1, 1);

        let origin_label = Label::new(Some(&pkg.origin));
        origin_label.set_halign(Align::Start);
        origin_label.set_xalign(0.0);
        origin_label.add_css_class("dim-label");
        grid.attach(&origin_label, 2, row_idx, 1, 1);

        let comment_label = Label::new(Some(&pkg.comment));
        comment_label.set_halign(Align::Start);
        comment_label.set_xalign(0.0);
        comment_label.add_css_class("dim-label");
        comment_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        comment_label.set_hexpand(true);
        grid.attach(&comment_label, 3, row_idx, 1, 1);
    }

    outer_box.append(&grid);
    scrolled.set_child(Some(&outer_box));
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
    theme_label.set_halign(Align::Start);
    main_box.append(&theme_label);

    let theme_box = Box::new(gtk4::Orientation::Horizontal, 12);
    let light_btn = Button::with_label("Light");
    let dark_btn = Button::with_label("Dark");
    let system_btn = Button::with_label("System");
    theme_box.append(&light_btn);
    theme_box.append(&dark_btn);
    theme_box.append(&system_btn);
    main_box.append(&theme_box);

    let ok_btn = Button::with_label("OK");
    ok_btn.set_halign(Align::End);
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

        let content_box = Box::new(gtk4::Orientation::Horizontal, 0);
        content_box.set_vexpand(true);
        main_box.append(&content_box);

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

        sidebar.append(&install_btn);
        sidebar.append(&remove_btn);
        sidebar.append(&orphans_btn);
        sidebar.append(&installed_btn);
        sidebar.append(&deps_btn);

        let separator = Separator::new(gtk4::Orientation::Horizontal);
        separator.set_margin_top(12);
        separator.set_margin_bottom(12);
        sidebar.append(&separator);

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

        sidebar.append(&prefs_btn);
        sidebar.append(&quit_btn);

        let package_area = Box::new(gtk4::Orientation::Vertical, 0);
        package_area.set_hexpand(true);
        package_area.set_vexpand(true);
        content_box.append(&package_area);

        let scrolled = ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_hexpand(true);
        package_area.append(&scrolled);

        let scrolled_clone = scrolled.clone();
        let scrolled_clone2 = scrolled.clone();
        let scrolled_clone3 = scrolled.clone();
        let scrolled_clone4 = scrolled.clone();

        // Current category and packages
        let current_category = Arc::new(Mutex::new("Installed Packages".to_string()));
        let all_packages = Arc::new(Mutex::new(list_installed_packages()));
        
        let search_entry_clone = search_entry.clone();
        let scrolled_for_search = scrolled.clone();
        let all_packages_for_search = all_packages.clone();
        let current_category_for_search = current_category.clone();
        
        search_entry.connect_changed(move |entry| {
            let search_text = entry.text().to_string();
            let category = current_category_for_search.lock().unwrap();
            let packages = all_packages_for_search.lock().unwrap();
            let filtered = filter_packages(&packages, &search_text);
            update_package_list(&scrolled_for_search, filtered, &category);
        });

        let all_packages_refresh = all_packages.clone();
        let current_category_refresh = current_category.clone();
        refresh_btn.connect_clicked(move |_| {
            let packages = list_installed_packages();
            *all_packages_refresh.lock().unwrap() = packages.clone();
            *current_category_refresh.lock().unwrap() = "Installed Packages".to_string();
            update_package_list(&scrolled_clone, packages, "Installed Packages");
        });

        let all_packages_installed = all_packages.clone();
        let current_category_installed = current_category.clone();
        installed_btn.connect_clicked(move |_| {
            let packages = list_installed_packages();
            *all_packages_installed.lock().unwrap() = packages.clone();
            *current_category_installed.lock().unwrap() = "Installed Packages".to_string();
            update_package_list(&scrolled_clone2, packages, "Installed Packages");
        });

        let all_packages_orphans = all_packages.clone();
        let current_category_orphans = current_category.clone();
        orphans_btn.connect_clicked(move |_| {
            let packages = list_orphan_packages();
            *all_packages_orphans.lock().unwrap() = packages.clone();
            *current_category_orphans.lock().unwrap() = "Orphan Packages".to_string();
            update_package_list(&scrolled_clone3, packages, "Orphan Packages");
        });

        let all_packages_deps = all_packages.clone();
        let current_category_deps = current_category.clone();
        deps_btn.connect_clicked(move |_| {
            let packages = list_dependencies();
            *all_packages_deps.lock().unwrap() = packages.clone();
            *current_category_deps.lock().unwrap() = "Dependencies".to_string();
            update_package_list(&scrolled_clone4, packages, "Dependencies");
        });

        let packages = list_installed_packages();
        update_package_list(&scrolled, packages, "Installed Packages");

        window.show();
    });

    app.run()
}
