use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Button, Label, ListBox, ScrolledWindow};
use libadwaita::HeaderBar;
use std::process::Command;

struct Package {
    name: String,
}

fn list_installed_packages() -> Vec<Package> {
    let output = Command::new("pkg")
        .arg("query")
        .arg("%n")
        .output();

    match output {
        Ok(result) if result.status.success() => {
            String::from_utf8_lossy(&result.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|name| Package {
                    name: name.trim().to_string(),
                })
                .collect()
        }
        _ => vec![],
    }
}

fn create_package_row(package: &Package) -> Box {
    let row_box = Box::new(gtk4::Orientation::Horizontal, 12);
    row_box.set_margin_top(12);
    row_box.set_margin_bottom(12);
    row_box.set_margin_start(12);
    row_box.set_margin_end(12);

    let name_label = Label::new(Some(&package.name));
    name_label.set_halign(gtk4::Align::Start);
    name_label.add_css_class("title-3");
    row_box.append(&name_label);

    let version_label = Label::new(Some("1.0.0")); // TODO: Get real version
    version_label.set_halign(gtk4::Align::End);
    row_box.append(&version_label);

    row_box
}

fn update_package_list(package_list: &ListBox) {
    // Clear existing rows
    while let Some(child) = package_list.first_child() {
        package_list.remove(&child);
    }

    // Add packages
    for pkg in list_installed_packages() {
        let row = create_package_row(&pkg);
        package_list.append(&row);
    }

    if package_list.first_child().is_none() {
        let no_packages = Label::new(Some("No packages found. Run 'pkg install' to add packages."));
        no_packages.set_halign(gtk4::Align::Center);
        no_packages.set_margin_top(24);
        no_packages.set_margin_bottom(24);
        no_packages.set_margin_start(24);
        no_packages.set_margin_end(24);
        package_list.append(&no_packages);
    }
}

fn main() -> glib::ExitCode {
    // Create a new application
    let app = Application::builder()
        .application_id("com.github.Olen1962.pkgui")
        .build();

    app.connect_activate(|app| {
        // Create main window
        let window = ApplicationWindow::builder()
            .application(app)
            .title("PkGUI - BSD Package Manager")
            .default_width(800)
            .default_height(600)
            .build();

        // Header bar
        let header = HeaderBar::builder()
            .title_widget(&Label::new(Some("PkGUI")))
            .show_end_title_buttons(true)
            .build();
        window.set_titlebar(Some(&header));

        // Main content
        let main_box = Box::new(gtk4::Orientation::Vertical, 12);
        main_box.set_margin_top(12);
        main_box.set_margin_bottom(12);
        main_box.set_margin_start(12);
        main_box.set_margin_end(12);
        window.set_child(Some(&main_box));

        // Welcome label
        let welcome = Label::new(Some("BSD Package Manager"));
        welcome.set_halign(gtk4::Align::Center);
        welcome.add_css_class("title-1");
        main_box.append(&welcome);

        // Package list
        let scrolled = ScrolledWindow::new();
        let package_list = ListBox::new();
        package_list.set_selection_mode(gtk4::SelectionMode::None);
        scrolled.set_child(Some(&package_list));
        main_box.append(&scrolled);

        // Action buttons
        let button_box = Box::new(gtk4::Orientation::Horizontal, 12);
        button_box.set_margin_top(12);
        button_box.set_margin_bottom(12);
        button_box.set_halign(gtk4::Align::Center);

        let refresh_btn = Button::with_label("Refresh");
        let install_btn = Button::with_label("Install");
        let remove_btn = Button::with_label("Remove");

        // Connect button signals
        let package_list_clone = package_list.clone();
        refresh_btn.connect_clicked(move |_| {
            update_package_list(&package_list_clone);
        });

        install_btn.connect_clicked(|_| {
            println!("Install button clicked");
        });

        remove_btn.connect_clicked(|_| {
            println!("Remove button clicked");
        });

        button_box.append(&refresh_btn);
        button_box.append(&install_btn);
        button_box.append(&remove_btn);
        main_box.append(&button_box);

        // Initial load
        update_package_list(&package_list);

        window.show();
    });

    app.run()
}
