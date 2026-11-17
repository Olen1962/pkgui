use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Button, Entry, Grid, GestureClick, Label, ScrolledWindow, Separator, Align};
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

fn search_available_packages(query: &str) -> Vec<Package> {
    if query.trim().is_empty() {
        return vec![];
    }
    
    let output = Command::new("pkg")
        .arg("search")
        .arg(query)
        .arg("-F")
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
            eprintln!("pkg search stderr: {}", String::from_utf8_lossy(&result.stderr));
            vec![]
        }
        Err(e) => {
            eprintln!("pkg search error: {}", e);
            vec![]
        }
    }
}

fn install_package(package_name: &str) -> Result<String, String> {
    let output = Command::new("pkg")
        .arg("install")
        .arg("-y")
        .arg(package_name)
        .output();

    match output {
        Ok(result) if result.status.success() => {
            Ok(format!("Package {} installed successfully", package_name))
        }
        Ok(result) => {
            Err(String::from_utf8_lossy(&result.stderr).to_string())
        }
        Err(e) => {
            Err(format!("Failed to install package: {}", e))
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

fn create_install_dialog(parent: &ApplicationWindow) {
    let dialog = gtk4::Window::builder()
        .title("Install Package")
        .transient_for(parent)
        .modal(true)
        .default_width(600)
        .default_height(500)
        .build();

    let main_box = Box::new(gtk4::Orientation::Vertical, 12);
    main_box.set_margin_top(12);
    main_box.set_margin_bottom(12);
    main_box.set_margin_start(12);
    main_box.set_margin_end(12);
    dialog.set_child(Some(&main_box));

    // Search entry
    let search_entry = Entry::builder()
        .placeholder_text("Search for packages...")
        .hexpand(true)
        .build();
    main_box.append(&search_entry);

    // Results area
    let scrolled = ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);
    main_box.append(&scrolled);

    // Status label
    let status_label = Label::new(Some("Enter a package name to search"));
    status_label.set_halign(Align::Start);
    status_label.add_css_class("dim-label");
    main_box.append(&status_label);

    // Install button (disabled by default)
    let install_btn = Button::with_label("Install");
    install_btn.set_sensitive(false);
    install_btn.set_halign(Align::End);
    main_box.append(&install_btn);

    // State
    let selected_package = Arc::new(Mutex::new(None::<Package>));
    let scrolled_clone = scrolled.clone();
    let status_label_for_search = status_label.clone();
    let install_btn_for_search = install_btn.clone();
    let selected_package_for_search = selected_package.clone();

    search_entry.connect_changed(move |entry| {
        let query = entry.text().to_string();
        if query.trim().is_empty() {
            let empty_box = Box::new(gtk4::Orientation::Vertical, 0);
            empty_box.set_margin_top(24);
            empty_box.set_margin_bottom(24);
            empty_box.set_margin_start(24);
            empty_box.set_margin_end(24);
            let label = Label::new(Some("Enter a package name to search"));
            label.set_halign(Align::Center);
            label.add_css_class("dim-label");
            empty_box.append(&label);
            scrolled_clone.set_child(Some(&empty_box));
            status_label_for_search.set_text("Enter a package name to search");
            install_btn_for_search.set_sensitive(false);
            *selected_package_for_search.lock().unwrap() = None;
            return;
        }

        let packages = search_available_packages(&query);
        
        if packages.is_empty() {
            let empty_box = Box::new(gtk4::Orientation::Vertical, 0);
            empty_box.set_margin_top(24);
            empty_box.set_margin_bottom(24);
            empty_box.set_margin_start(24);
            empty_box.set_margin_end(24);
            let label = Label::new(Some("No packages found"));
            label.set_halign(Align::Center);
            label.add_css_class("dim-label");
            empty_box.append(&label);
            scrolled_clone.set_child(Some(&empty_box));
            status_label_for_search.set_text("No packages found");
            install_btn_for_search.set_sensitive(false);
            *selected_package_for_search.lock().unwrap() = None;
            return;
        }

        // Create grid for results
        let outer_box = Box::new(gtk4::Orientation::Vertical, 0);
        outer_box.set_margin_top(12);
        outer_box.set_margin_bottom(12);
        outer_box.set_margin_start(12);
        outer_box.set_margin_end(12);

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

            // Make the entire row clickable using GestureClick
            let row_box = Box::new(gtk4::Orientation::Horizontal, 0);
            row_box.set_halign(Align::Fill);
            row_box.set_vexpand(false);
            
            let selected_for_row = selected_package_for_search.clone();
            let install_btn_for_row = install_btn_for_search.clone();
            let status_for_row = status_label_for_search.clone();
            let pkg_for_row = pkg.clone();
            
            let gesture = GestureClick::new();
            gesture.connect_pressed(move |_, _, _, _| {
                *selected_for_row.lock().unwrap() = Some(pkg_for_row.clone());
                install_btn_for_row.set_sensitive(true);
                status_for_row.set_text(&format!("Selected: {}", pkg_for_row.name));
            });
            row_box.add_controller(gesture);
            
            // Add row to grid
            grid.attach(&row_box, 0, row_idx, 4, 1);
        }

        outer_box.append(&grid);
        scrolled_clone.set_child(Some(&outer_box));
        status_label_for_search.set_text(&format!("Found {} packages", packages.len()));
    });

    let dialog_clone = dialog.clone();
    let selected_for_install = selected_package_for_search.clone();
    let status_for_install = status_label_for_search.clone();
    let install_btn_for_install = install_btn_for_search.clone();
    
    install_btn_for_install.connect_clicked(move |_| {
        if let Some(pkg) = selected_for_install.lock().unwrap().as_ref() {
            match install_package(&pkg.name) {
                Ok(msg) => {
                    status_for_install.set_text(&msg);
                    dialog_clone.close();
                }
                Err(err) => {
                    status_for_install.set_text(&format!("Error: {}", err));
                }
            }
        }
    });

    dialog.show();
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

        let window_clone3 = window.clone();
        install_btn.connect_clicked(move |_| {
            create_install_dialog(&window_clone3);
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
