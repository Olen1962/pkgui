use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Button, Label, ScrolledWindow};
use libadwaita::{Application as AdwApplication, HeaderBar};

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
            .show_title_buttons(true)
            .build();
        window.set_titlebar(Some(&header));

        // Main content
        let main_box = Box::new(gtk4::Orientation::Vertical, 12);
        window.set_child(Some(&main_box));

        // Welcome label
        let welcome = Label::new(Some("BSD Package Manager"));
        welcome.set_halign(gtk4::Align::Center);
        welcome.add_css_class("title-1");
        main_box.append(&welcome);

        // Package list placeholder
        let scrolled = ScrolledWindow::new();
        let package_list = Label::new(Some("Package list will appear here\n\nFeatures:\n- List installed packages\n- Search packages\n- Install/Remove packages\n- System updates"));
        package_list.set_halign(gtk4::Align::Start);
        package_list.set_valign(gtk4::Align::Start);
        package_list.set_margin_all(24);
        scrolled.set_child(Some(&package_list));
        main_box.append(&scrolled);

        // Action buttons
        let button_box = Box::new(gtk4::Orientation::Horizontal, 12);
        button_box.set_margin_all(12);
        button_box.set_halign(gtk4::Align::Center);

        let refresh_btn = Button::with_label("Refresh");
        let install_btn = Button::with_label("Install");
        let remove_btn = Button::with_label("Remove");

        button_box.append(&refresh_btn);
        button_box.append(&install_btn);
        button_box.append(&remove_btn);
        main_box.append(&button_box);

        window.show();
    });

    app.run()
}
