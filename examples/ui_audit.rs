//! Opt-in native UI smoke test. See docs/ui-preview.md for the isolated launch.
#![allow(deprecated)]
use adw::prelude::*;
use gtk::{gdk, gio, glib, gsk};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tsukimi::client::{
    Account, ServerRoute, account::ServerType, jellyfin_client::JELLYFIN_CLIENT,
};

thread_local! {
    static MISSING_ICONS: RefCell<std::collections::BTreeSet<String>> = RefCell::default();
}

fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut result = vec![root.clone()];
    let mut child = root.first_child();
    while let Some(widget) = child {
        result.extend(descendants(&widget));
        child = widget.next_sibling();
    }
    result
}

fn find(root: &gtk::Widget, predicate: impl Fn(&gtk::Widget) -> bool) -> gtk::Widget {
    descendants(root)
        .into_iter()
        .find(predicate)
        .expect("Expected UI control")
}

fn output_dir() -> PathBuf {
    let path = std::env::var_os("TSUKIMI_UI_AUDIT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("tsukimi-ui-audit"));
    std::path::absolute(path).unwrap()
}

fn screenshot(window: &gtk::Window, name: &str) {
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(window)).snapshot(
        &snapshot,
        window.width() as f64,
        window.height() as f64,
    );
    let node = snapshot.to_node().expect("Nonblank window");
    let renderer = gsk::Renderer::for_surface(&window.surface().unwrap()).unwrap();
    let texture = renderer.render_texture(&node, None);
    let theme = std::env::var("AUDIT_THEME").unwrap_or_else(|_| "3".into());
    texture
        .save_to_png(output_dir().join(format!("{theme}-{name}.png")))
        .unwrap();
    renderer.unrealize();
    println!("CHECK {name}: {}x{}", window.width(), window.height());
    check_widgets(window.upcast_ref());
    if std::env::var_os("TSUKIMI_UI_AUDIT_TRACE").is_some() && name == "movie" {
        for widget in descendants(window.upcast_ref())
            .into_iter()
            .filter(|w| w.is::<gtk::Box>() && w.is_mapped() && w.width() > 0)
        {
            let (min, natural, _, _) = widget.layout_manager().unwrap().measure(
                &widget,
                gtk::Orientation::Vertical,
                widget.width(),
            );
            if min > natural {
                eprintln!(
                    "INVALID MEASURE {:?} {:?} {}x{} {min}/{natural} parent={:?}",
                    widget.as_ptr(),
                    widget.css_classes(),
                    widget.width(),
                    widget.height(),
                    widget.parent().map(|p| p.css_name())
                );
                let mut child = widget.first_child();
                while let Some(w) = child {
                    eprintln!(
                        "  CHILD {} {:?} visible={} measure={:?}",
                        w.type_().name(),
                        w.css_classes(),
                        w.is_visible(),
                        w.measure(gtk::Orientation::Vertical, w.width())
                    );
                    child = w.next_sibling();
                }
            }
        }
    }
}

#[derive(Default, Debug, PartialEq)]
struct Surface {
    radii: Vec<[f32; 4]>,
    colors: Vec<gdk::RGBA>,
}

fn collect_surface(node: &gsk::RenderNode, surface: &mut Surface) {
    if let Some(n) = node.downcast_ref::<gsk::BorderNode>() {
        surface.radii.push(n.outline().corner().map(|s| s.width()));
    } else if let Some(n) = node.downcast_ref::<gsk::ColorNode>() {
        surface.colors.push(n.color());
    } else if let Some(n) = node.downcast_ref::<gsk::RoundedClipNode>() {
        surface.radii.push(n.clip().corner().map(|s| s.width()));
        collect_surface(&n.child(), surface);
    } else if let Some(n) = node.downcast_ref::<gsk::ContainerNode>() {
        for i in 0..n.n_children() {
            collect_surface(&n.child(i), surface);
        }
    } else if let Some(n) = node.downcast_ref::<gsk::TransformNode>() {
        collect_surface(&n.child(), surface);
    } else if let Some(n) = node.downcast_ref::<gsk::ClipNode>() {
        collect_surface(&n.child(), surface);
    } else if let Some(n) = node.downcast_ref::<gsk::OpacityNode>() {
        collect_surface(&n.child(), surface);
    } else if let Some(n) = node.downcast_ref::<gsk::ShadowNode>() {
        collect_surface(&n.child(), surface);
    }
}

fn surface(widget: &gtk::Widget) -> Surface {
    let snapshot = gtk::Snapshot::new();
    snapshot.render_background(&widget.style_context(), 0.0, 0.0, 400.0, 100.0);
    snapshot.render_frame(&widget.style_context(), 0.0, 0.0, 400.0, 100.0);
    let mut result = Surface::default();
    if let Some(node) = snapshot.to_node() {
        collect_surface(&node, &mut result);
    }
    result
}

fn check_widgets(root: &gtk::Widget) {
    let theme = gtk::IconTheme::for_display(&gdk::Display::default().unwrap());
    let is_dark = adw::StyleManager::default().is_dark();
    for widget in descendants(root).into_iter().filter(|w| w.is_mapped()) {
        if std::env::var_os("TSUKIMI_UI_AUDIT_TRACE").is_some()
            && widget.has_css_class("suggested-action")
        {
            eprintln!(
                "ACTION {} {} {:?} sensitive={} fg={} accent={:?} background={:?}",
                widget.type_().name(),
                widget.css_name(),
                widget.css_classes(),
                widget.is_sensitive(),
                widget.style_context().color(),
                widget.style_context().lookup_color("accent_bg_color"),
                surface(&widget).colors
            );
        }
        if widget.css_name() == "sheet" {
            let bounds = widget.compute_bounds(root).expect("Dialog bounds");
            assert!(
                bounds.x() >= -1.0
                    && bounds.y() >= -1.0
                    && bounds.x() + bounds.width() <= root.width() as f32 + 1.0
                    && bounds.y() + bounds.height() <= root.height() as f32 + 1.0,
                "Dialog extends outside its parent window: {bounds:?}"
            );
        }
        if widget.type_().name() == "FiltersRow" {
            for label in descendants(&widget)
                .into_iter()
                .filter_map(|w| w.downcast::<gtk::Label>().ok())
                .filter(|label| label.has_css_class("title"))
            {
                let (_, natural, _, _) = label.measure(gtk::Orientation::Horizontal, -1);
                assert!(
                    label.width() >= natural,
                    "Cramped filter title: {}",
                    label.text()
                );
            }
        }
        if widget.has_css_class("boxed-list") && widget.is::<gtk::ListBox>()
            || widget.has_css_class("card")
            || widget.has_css_class("mpv-sidebar-panel")
            || widget.has_css_class("account-settings")
            || widget.css_name() == "sheet"
        {
            let radii = surface(&widget).radii;
            if !radii.is_empty()
                && (widget.css_name() != "sheet"
                    || widget
                        .parent()
                        .is_some_and(|p| p.css_name() == "floating-sheet"))
            {
                assert!(
                    radii
                        .iter()
                        .any(|r| r.iter().all(|v| (*v - 14.0).abs() < 0.01)),
                    "Wrong frame radius: {} {:?} {radii:?}",
                    widget.css_name(),
                    widget.css_classes()
                );
            }
        }
        if widget.has_css_class("navigation-switcher") && widget.width() > 0 && widget.height() > 0
        {
            let buttons: Vec<_> = descendants(&widget)
                .into_iter()
                .filter(|w| w.is::<gtk::Button>() && w.is_mapped() && w.height() > 0)
                .collect();
            assert!(buttons.len() >= 3);
            for button in buttons {
                assert_eq!(
                    button.compute_bounds(&button).unwrap().height().round() as i32,
                    36,
                    "Shared navigation tab height"
                );
                for label in descendants(&button)
                    .into_iter()
                    .filter_map(|w| w.downcast::<gtk::Label>().ok())
                {
                    let (_, natural, _, _) = label.measure(gtk::Orientation::Horizontal, -1);
                    assert!(
                        label.width() >= natural,
                        "Clipped navigation label: {}",
                        label.text()
                    );
                }
            }
        }
        if widget.has_css_class("library-toolbar") {
            let toolbar = widget.downcast_ref::<gtk::CenterBox>().unwrap();
            let summary = toolbar.start_widget().unwrap();
            let tabs = toolbar
                .center_widget()
                .expect("Library navigation in toolbar");
            let actions = toolbar.end_widget().unwrap();
            let summary_bounds = summary.compute_bounds(&widget).unwrap();
            let tabs_bounds = tabs.compute_bounds(&widget).unwrap();
            let actions_bounds = actions.compute_bounds(&widget).unwrap();
            for bounds in [summary_bounds, actions_bounds] {
                if bounds.width() > 0.0 {
                    assert!(
                        ((bounds.y() + bounds.height() / 2.0)
                            - (tabs_bounds.y() + tabs_bounds.height() / 2.0))
                            .abs()
                            <= 1.0,
                        "Library controls must share one row"
                    );
                }
            }
            assert!(summary_bounds.x() + summary_bounds.width() + 8.0 <= tabs_bounds.x() + 1.0);
            assert!(tabs_bounds.x() + tabs_bounds.width() + 8.0 <= actions_bounds.x() + 1.0);
            let switcher = find(&tabs, |child| child.has_css_class("library-tabs"));
            let (_, natural, _, _) = switcher.measure(gtk::Orientation::Horizontal, -1);
            assert!(tabs.width() >= natural, "Library navigation clipped");
        }
        if widget.has_css_class("media-card-title") || widget.has_css_class("person-card-title") {
            let color = widget.style_context().color();
            assert_eq!(color.red() > 0.5, is_dark, "Poster title contrast: {color}");
        }
        if widget.type_().name() == "TuListItem" {
            let image = find(&widget, |child| child.has_css_class("media-card-image"));
            let bounds = image.compute_bounds(&image).expect("Poster bounds");
            let expected = (image.width_request(), image.height_request());
            assert_eq!(
                (
                    bounds.width().round() as i32,
                    bounds.height().round() as i32
                ),
                expected,
                "Poster enlarged by its content"
            );
            assert!(
                expected.0 <= 352 && expected.1 <= 264,
                "Oversized poster: {expected:?}"
            );
        }
        if widget.has_css_class("album-cover") {
            assert_eq!(
                (widget.width(), widget.height()),
                (176, 176),
                "Album cover size"
            );
        }
        if widget.has_css_class("mpv-sidebar-panel") {
            assert!(
                surface(&widget).colors.iter().any(|color| {
                    color.alpha() > 0.99
                        && color.red() < 0.2
                        && color.green() < 0.2
                        && color.blue() < 0.2
                }),
                "Player utility panel must remain dark"
            );
            for label in descendants(&widget)
                .into_iter()
                .filter_map(|child| child.downcast::<gtk::Label>().ok())
                .filter(|label| label.is_mapped())
            {
                assert!(
                    label.style_context().color().red() > 0.5,
                    "Dark player text: {}",
                    label.text()
                );
            }
        }
        if widget.is::<gtk::Button>() && widget.width() > 0 && widget.height() > 0 {
            let menu = widget
                .parent()
                .filter(|parent| parent.is::<gtk::MenuButton>());
            let tool = widget.has_css_class("circular-icon-button")
                || widget.has_css_class("mpv-control-button")
                || widget.has_css_class("carousel-arrow-button")
                || widget.has_css_class("media-card-play-button")
                || widget.has_css_class("hero-favorite-button")
                || menu.as_ref().is_some_and(|parent| {
                    parent.has_css_class("circular-icon-button")
                        || parent.has_css_class("mpv-control-button")
                        || parent.has_css_class("hero-more-button")
                });
            if tool {
                let bounds = widget.compute_bounds(&widget).expect("Tool bounds");
                let size = if widget.has_css_class("mpv-play-button") {
                    48
                } else {
                    40
                };
                assert_eq!(
                    (
                        bounds.width().round() as i32,
                        bounds.height().round() as i32
                    ),
                    (size, size),
                    "Compact tool target: {:?}",
                    widget.css_classes()
                );
            }
            if widget.has_css_class("hero-play-button") {
                assert_eq!(
                    widget.compute_bounds(&widget).unwrap().height().round() as i32,
                    40,
                    "Detail play button height"
                );
            }
        }
        if widget.has_css_class("suggested-action")
            && widget.is::<adw::ButtonRow>()
            && widget.is_sensitive()
            && widget
                .ancestor(adw::PreferencesWindow::static_type())
                .is_some()
        {
            let accent = widget
                .style_context()
                .lookup_color("accent_bg_color")
                .unwrap();
            assert!(
                surface(&widget)
                    .colors
                    .iter()
                    .any(|color| color.alpha() > 0.99
                        && (color.red() - accent.red()).abs() < 0.15
                        && (color.green() - accent.green()).abs() < 0.15
                        && (color.blue() - accent.blue()).abs() < 0.15),
                "Settings action lost its accent background"
            );
        }
        if widget.has_css_class("suggested-action")
            && !widget.is_sensitive()
            && widget
                .ancestor(adw::PreferencesWindow::static_type())
                .is_some()
        {
            for label in descendants(&widget)
                .into_iter()
                .filter_map(|w| w.downcast::<gtk::Label>().ok())
            {
                let color = label.style_context().color();
                assert_eq!(
                    color.red() > 0.5,
                    is_dark,
                    "Disabled action contrast: {color}"
                );
            }
        }
        if widget.has_css_class("media-info-streams") && widget.width() > 0 {
            let (minimum, natural, _, _) =
                widget.measure(gtk::Orientation::Vertical, widget.width());
            assert!(natural >= minimum, "Inconsistent media card height request");
            let widths: Vec<_> = descendants(&widget)
                .into_iter()
                .filter(|w| w.has_css_class("media-info-card"))
                .map(|w| w.width())
                .collect();
            assert!(
                widths.iter().all(|width| *width == widths[0]),
                "Unequal media card widths"
            );
        }
        if widget.has_css_class("songwidget") {
            for button in descendants(&widget).into_iter().filter(|w| {
                w.is::<gtk::Button>() && w.is_mapped() && w.has_css_class("image-button")
            }) {
                let bounds = button.compute_bounds(&button).expect("Song tool bounds");
                assert_eq!(
                    (
                        bounds.width().round() as i32,
                        bounds.height().round() as i32
                    ),
                    (40, 40),
                    "Song tool size"
                );
            }
        }
        if let Some(image) = widget.downcast_ref::<gtk::Image>()
            && let Some(icon) = image.icon_name()
        {
            if !theme.has_icon(&icon) {
                MISSING_ICONS.with(|icons| {
                    icons.borrow_mut().insert(icon.to_string());
                });
                eprintln!("MISSING ICON {icon}");
            }
        }
        if widget.is::<gtk::WindowControls>() {
            for button in descendants(&widget)
                .into_iter()
                .filter(|w| w.is::<gtk::Button>() && w.is_mapped())
            {
                let bounds = button
                    .compute_bounds(&button)
                    .expect("Window-control bounds");
                assert_eq!(
                    (
                        bounds.width().round() as i32,
                        bounds.height().round() as i32
                    ),
                    (16, 16),
                    "Traffic light size"
                );
            }
        }
    }
}

fn fixtures() -> Vec<Value> {
    [
        ("movie", "Movie", "Fixture Movie With A Long Title"),
        ("series", "Series", "Fixture Series"),
        ("album", "MusicAlbum", "Fixture Album"),
        ("person", "Person", "Fixture Person"),
        ("episode", "Episode", "Fixture Episode"),
        ("audio", "Audio", "Fixture Song"),
    ].into_iter().map(|(id, kind, name)| json!({
        "Id": id, "Type": kind, "Name": name, "ProductionYear": 2026,
        "IndexNumber": 1, "ParentIndexNumber": 1,
        "SeriesId": if kind == "Episode" { Some("series") } else { None },
        "SeriesName": if kind == "Episode" { Some("Fixture Series") } else { None },
        "SeasonId": "season", "CommunityRating": 8.4,
        "RunTimeTicks": 54000000000_u64, "PrimaryImageAspectRatio": 0.667,
        "ImageTags": {"Primary": "fixture", "Thumb": "fixture", "Backdrop": "fixture"},
        "BackdropImageTags": ["fixture"], "Overview": "Local fixture for native UI checks.",
        "Path": "/fixture/media.mkv", "DateCreated": "2026-01-01T00:00:00Z",
        "PremiereDate": "2026-01-01T00:00:00Z", "Genres": ["Drama"],
        "GenreItems": [{"Id": "genre", "Name": "Drama"}], "Taglines": ["Fixture tagline"],
        "AlbumArtist": "Fixture Artist", "AlbumArtists": [{"Id": "person", "Name": "Fixture Person"}],
        "UserData": {"Played": false, "IsFavorite": true, "PlayedPercentage": 15.0,
            "PlaybackPositionTicks": 90000000, "UnplayedItemCount": 4}
    })).collect()
}

fn mock_response(target: &str) -> Value {
    let url = url::Url::parse(&format!("http://localhost{target}")).unwrap();
    let path = url
        .path()
        .trim_start_matches('/')
        .trim_start_matches("emby/");
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    let mut items = fixtures();
    if path == "Users/audit" {
        return json!({"Policy": {"IsAdministrator": true}});
    }
    if path == "System/Info" {
        return json!({"ServerName": "UI Audit", "Version": "Fixture", "LocalAddress": "", "WanAddress": ""});
    }
    if path == "System/ActivityLog/Entries" {
        return json!({"Items": []});
    }
    if path == "ScheduledTasks" {
        return json!([]);
    }
    if path.ends_with("/RemoteImages") {
        return json!({"Images": [], "TotalRecordCount": 0, "Providers": []});
    }
    if path.ends_with("/Views") {
        items = vec![
            json!({"Id": "library", "Name": "Fixture Library", "Type": "CollectionFolder", "CollectionType": "movies"}),
        ];
    } else if path.ends_with("/ExternalIdInfos") || path.ends_with("/Images") {
        return json!([]);
    } else if path.ends_with("/PlaybackInfo") {
        return json!({"PlaySessionId": "fixture", "MediaSources": [{
            "Id": "fixture", "Name": "1080p", "Container": "mkv", "Size": 1000000000,
            "Path": "/fixture/media.mkv", "RunTimeTicks": 54000000000_u64,
            "MediaStreams": [
                {"Type": "Video", "Codec": "h264", "Index": 0, "IsExternal": false,
                 "DisplayTitle": "1080p H264", "Width": 1920, "Height": 1080},
                {"Type": "Audio", "Codec": "aac", "Index": 1, "IsExternal": false,
                 "DisplayTitle": "Stereo AAC", "Channels": 2, "SampleRate": 48000}
            ]
        }]});
    } else if (path.starts_with("Items/") && path.split('/').count() == 2
        || path.starts_with("Users/audit/Items/") && path.split('/').count() == 4)
        && matches!(
            path.rsplit('/').next().unwrap(),
            "movie" | "series" | "album" | "person" | "episode" | "audio" | "season"
        )
    {
        return items
            .into_iter()
            .find(|i| i["Id"] == path.rsplit('/').next().unwrap())
            .unwrap_or_else(|| json!({"Id": "season", "Name": "Season 1", "Type": "Season"}));
    } else if path.contains("Filters") {
        return json!({"Genres": ["Drama"], "Tags": ["Fixture"], "Years": [2026], "OfficialRatings": ["PG"]});
    } else if path.contains("/Seasons") {
        items =
            vec![json!({"Id": "season", "Name": "Season 1", "Type": "Season", "IndexNumber": 1})];
    } else if path.contains("/Episodes") || path.contains("NextUp") {
        items.retain(|i| i["Type"] == "Episode");
    } else if path == "Persons" {
        items.retain(|i| i["Type"] == "Person");
    } else if let Some(types) = query.get("IncludeItemTypes") {
        items.retain(|i| types.split(',').any(|kind| i["Type"] == kind));
    }
    if query
        .get("SearchTerm")
        .is_some_and(|term| term == "no-match")
    {
        items.clear();
    }
    if path.ends_with("/Latest") {
        return Value::Array(items);
    }
    json!({"TotalRecordCount": items.len(), "Items": items})
}

fn serve(mut stream: TcpStream, image: Arc<Vec<u8>>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut request = Vec::new();
    let mut buffer = [0; 8192];
    while let Ok(n) = stream.read(&mut buffer) {
        if n == 0 {
            return;
        }
        request.extend_from_slice(&buffer[..n]);
        if request.windows(4).any(|s| s == b"\r\n\r\n") {
            break;
        }
        if request.len() > 65536 {
            return;
        }
    }
    let request = String::from_utf8_lossy(&request);
    let target = request
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .nth(1)
        .unwrap_or("/");
    let (content_type, body) = if target.split('?').next().unwrap().contains("/Images/") {
        ("image/png", (*image).clone())
    } else {
        (
            "application/json",
            serde_json::to_vec(&mock_response(target)).unwrap(),
        )
    };
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(&body);
}

fn mock_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/", listener.local_addr().unwrap());
    let image = image::RgbImage::from_fn(320, 480, |x, y| {
        image::Rgb([
            if x < 160 { 45 } else { 180 },
            if y < 240 { 150 } else { 55 },
            100,
        ])
    });
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let image = Arc::new(bytes.into_inner());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let image = image.clone();
            std::thread::spawn(move || serve(stream, image));
        }
    });
    address
}

fn seed_session(address: &str) {
    let account = Account {
        servername: "UI Audit".into(),
        username: "fixture".into(),
        user_id: "audit".into(),
        server_type: Some(ServerType::Jellyfin),
        routes: vec![ServerRoute {
            name: "Local fixture".into(),
            url: address.into(),
        }],
        default_route: Some("Local fixture".into()),
        selected_route: Some("Local fixture".into()),
        ..Account::default()
    };
    gio::Settings::new(tsukimi::APP_ID)
        .set_string("accounts", &serde_json::to_string(&vec![&account]).unwrap())
        .unwrap();
    tsukimi::client::runtime::runtime()
        .block_on(JELLYFIN_CLIENT.init(&account))
        .unwrap();
    let mut session = (**JELLYFIN_CLIENT.session()).clone();
    // An absolute cache suffix keeps fixture data away from the user's media cache.
    session.server_name_hash = output_dir()
        .join("fixture-cache")
        .to_string_lossy()
        .into_owned();
    JELLYFIN_CLIENT.session.store(Arc::new(session));
}

fn click_card(main: &gtk::Widget, kind: &str) {
    let card = find(main, |w| {
        w.type_().name() == "TuListItem"
            && w.is_mapped()
            && w.property::<glib::Object>("item")
                .property::<String>("item-type")
                == kind
    });
    scroll_into_view(&card);
    glib::timeout_add_local_once(Duration::from_millis(80), move || {
        let controllers = card.observe_controllers();
        for i in 0..controllers.n_items() {
            if let Some(gesture) = controllers.item(i).and_downcast::<gtk::GestureClick>()
                && gesture.button() == 1
            {
                gesture.emit_by_name::<()>("released", &[&1_i32, &10_f64, &10_f64]);
                return;
            }
        }
        panic!("Card has no activation gesture");
    });
}

fn card_action(main: &gtk::Widget, kind: &str, action: &str) {
    let card = find(main, |w| {
        w.type_().name() == "TuListItem"
            && w.is_mapped()
            && !w
                .property::<glib::Object>("item")
                .property::<bool>("is-resume")
            && w.property::<glib::Object>("item")
                .property::<String>("item-type")
                == kind
    });
    scroll_into_view(&card);
    let action = action.to_owned();
    glib::timeout_add_local_once(Duration::from_millis(80), move || {
        // Context actions are installed when the real secondary-click menu opens.
        let controllers = card.observe_controllers();
        for i in 0..controllers.n_items() {
            if let Some(gesture) = controllers.item(i).and_downcast::<gtk::GestureClick>()
                && gesture.button() == 3
            {
                gesture.emit_by_name::<()>("released", &[&1_i32, &10_f64, &10_f64]);
            }
        }
        for popover in descendants(&card)
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Popover>().ok())
        {
            popover.popdown();
        }
        card.activate_action(&action, None).unwrap();
    });
}

fn scroll_into_view(widget: &gtk::Widget) {
    let mut parent = widget.parent();
    while let Some(ancestor) = parent {
        if let Some(scroll) = ancestor.downcast_ref::<gtk::ScrolledWindow>()
            && let Some(bounds) = widget.compute_bounds(scroll)
        {
            for (adjustment, start, size, page) in [
                (
                    scroll.hadjustment(),
                    bounds.x(),
                    bounds.width(),
                    scroll.width(),
                ),
                (
                    scroll.vadjustment(),
                    bounds.y(),
                    bounds.height(),
                    scroll.height(),
                ),
            ] {
                let delta = if start < 0.0 {
                    start
                } else {
                    (start + size - page as f32).max(0.0)
                };
                adjustment.set_value((adjustment.value() + delta as f64).clamp(
                    adjustment.lower(),
                    (adjustment.upper() - adjustment.page_size()).max(adjustment.lower()),
                ));
            }
        }
        parent = ancestor.parent();
    }
}

fn scroll_to_bottom(root: &gtk::Widget) {
    for scroll in descendants(root)
        .into_iter()
        .filter(|widget| widget.is_mapped())
        .filter_map(|widget| widget.downcast::<gtk::ScrolledWindow>().ok())
    {
        let adjustment = scroll.vadjustment();
        adjustment.set_value((adjustment.upper() - adjustment.page_size()).max(adjustment.lower()));
    }
}

fn dialog(main: &gtk::Widget) -> adw::Dialog {
    find(main, |w| w.is::<adw::Dialog>() && w.is_mapped())
        .downcast()
        .unwrap()
}

fn search(main: &gtk::Widget, text: &str) {
    let entry = find(main, |w| w.is::<gtk::SearchEntry>() && w.is_mapped())
        .downcast::<gtk::SearchEntry>()
        .unwrap();
    entry.set_text(text);
    entry.emit_by_name::<()>("activate", &[]);
}

fn library_tab(main: &gtk::Widget, tab: &str) {
    let switcher = find(main, |w| w.has_css_class("library-tabs") && w.is_mapped())
        .downcast::<gtk::StackSwitcher>()
        .unwrap();
    let stack = switcher.stack().unwrap();
    let title = stack
        .page(&stack.child_by_name(tab).unwrap())
        .title()
        .unwrap();
    find(switcher.upcast_ref(), |widget| {
        widget.is::<gtk::ToggleButton>()
            && descendants(widget).into_iter().any(|child| {
                child
                    .downcast_ref::<gtk::Label>()
                    .is_some_and(|label| label.text() == title)
            })
    })
    .downcast::<gtk::ToggleButton>()
    .unwrap()
    .emit_clicked();
    assert_eq!(stack.visible_child_name().as_deref(), Some(tab));
}

fn settings_window() -> adw::PreferencesWindow {
    gtk::Window::list_toplevels()
        .into_iter()
        .find(|w| w.type_().name() == "AccountSettings" && w.is_visible())
        .unwrap()
        .downcast()
        .unwrap()
}

fn player_tab(main: &gtk::Widget, action: &str) {
    let tabs: Vec<_> = descendants(main)
        .into_iter()
        .filter(|w| w.has_css_class("mpv-sidebar-tab") && w.is_mapped())
        .collect();
    assert_eq!(tabs.len(), 3);
    let active: Vec<_> = tabs.iter().filter(|w| w.has_css_class("active")).collect();
    assert_eq!(active.len(), 1);
    assert_eq!(
        active[0]
            .downcast_ref::<gtk::Button>()
            .unwrap()
            .action_name()
            .as_deref(),
        Some(action)
    );
}

fn switch_theme(theme: i32) {
    let switcher = glib::Object::with_type(glib::Type::from_name("ThemeSwitcher").unwrap())
        .downcast::<gtk::Widget>()
        .unwrap();
    switcher
        .activate_action("app.color-scheme", Some(&theme.to_variant()))
        .unwrap();
}

fn main() {
    assert!(
        std::env::args().any(|arg| arg == "--ui-preview"),
        "Audit must run in isolated preview mode"
    );
    std::fs::create_dir_all(output_dir()).unwrap();
    let address = mock_server();
    gtk::init().unwrap();
    if std::env::var_os("TSUKIMI_UI_AUDIT_TRACE").is_some() {
        glib::log_set_handler(
            Some("Gtk"),
            glib::LogLevels::LEVEL_CRITICAL,
            false,
            false,
            |_, _, message| {
                eprintln!(
                    "GTK CRITICAL {message}\n{}",
                    std::backtrace::Backtrace::force_capture()
                )
            },
        );
    }
    gtk::Settings::default()
        .unwrap()
        .set_property("gtk-enable-animations", false);
    let stage = Cell::new(0);
    let ticks = Cell::new(0);
    let poster_actions_visible = Cell::new(false);
    let library_toolbar_stage = Cell::new(0);
    let main_navigation = RefCell::new(None::<Surface>);
    glib::timeout_add_local(Duration::from_millis(900), move || {
        ticks.set(ticks.get() + 1);
        assert!(ticks.get() < 100, "UI audit timed out");
        let Some(main) = gtk::Window::list_toplevels()
            .into_iter()
            .find(|w| w.type_().name() == "AppWindow" && w.is_visible() && w.opacity() > 0.9)
            .and_downcast::<tsukimi::Window>()
        else {
            return glib::ControlFlow::Continue;
        };
        let root = main.upcast_ref::<gtk::Widget>();
        match stage.get() {
            0 => {
                for (resource, source) in [
                    (
                        "style.css",
                        include_bytes!("../resources/style.css").as_slice(),
                    ),
                    (
                        "theme-dark.css",
                        include_bytes!("../resources/style-dark.css").as_slice(),
                    ),
                    (
                        "style-settings.css",
                        include_bytes!("../resources/style-settings.css").as_slice(),
                    ),
                    (
                        "ui/window.ui",
                        include_bytes!("../resources/ui/window.ui").as_slice(),
                    ),
                ] {
                    let runtime = gio::resources_lookup_data(
                        &format!("/moe/tsuna/tsukimi/{resource}"),
                        gio::ResourceLookupFlags::NONE,
                    )
                    .unwrap();
                    assert_eq!(
                        runtime.as_ref(),
                        source,
                        "Stale runtime resource: {resource}"
                    );
                }
                println!("PASS runtime resources match the current sources");
                let theme = std::env::var("AUDIT_THEME")
                    .unwrap_or_else(|_| "3".into())
                    .parse()
                    .unwrap();
                gio::Settings::new(tsukimi::APP_ID)
                    .set_int("main-theme", theme)
                    .unwrap();
                root.activate_action("win.settings", None).unwrap();
            }
            1..=5 => {
                let settings = settings_window();
                let pages = ["General", "Account", "media", "Others", "Shortcuts"];
                let page = pages[(stage.get() - 1) as usize];
                screenshot(settings.upcast_ref(), &format!("settings-{page}"));
                if stage.get() < 5 {
                    settings.set_visible_page_name(pages[stage.get() as usize]);
                } else {
                    settings.set_visible_page_name("General");
                    settings.set_default_size(500, 600);
                }
            }
            6 => {
                screenshot(settings_window().upcast_ref(), "settings-narrow");
                settings_window().close();
                screenshot(main.upcast_ref(), "no-server");
                seed_session(&address);
                main.homepage();
            }
            7 => {
                screenshot(main.upcast_ref(), "home");
                let nav = find(root, |w| w.has_css_class("top-navigation"));
                *main_navigation.borrow_mut() = Some(surface(&nav));
                main.likedpage();
            }
            8 => {
                screenshot(main.upcast_ref(), "favorites");
                main.recommendpage();
            }
            9 => {
                screenshot(main.upcast_ref(), "recommend");
                main.searchpage();
            }
            10 => {
                screenshot(main.upcast_ref(), "search");
                search(root, "Fixture");
            }
            11 => {
                screenshot(main.upcast_ref(), "search-results");
                search(root, "no-match");
            }
            12 => {
                screenshot(main.upcast_ref(), "search-empty");
                main.homepage();
            }
            13 => {
                let buttons: Vec<_> = descendants(root)
                    .into_iter()
                    .filter(|w| w.has_css_class("media-card-play-button"))
                    .collect();
                if !poster_actions_visible.replace(true) {
                    assert!(!buttons.is_empty(), "Missing poster play controls");
                    for button in &buttons {
                        button.set_visible(true);
                    }
                    return glib::ControlFlow::Continue;
                }
                screenshot(main.upcast_ref(), "poster-actions");
                for button in buttons {
                    button.set_visible(false);
                }
                let section = find(root, |w| {
                    w.type_().name() == "HortuScrolled"
                        && w.property::<String>("title").contains("Fixture Library")
                });
                find(&section, |w| {
                    w.downcast_ref::<gtk::Button>()
                        .is_some_and(|b| b.icon_name().as_deref() == Some("go-next-symbolic"))
                        && w.is_mapped()
                })
                .downcast::<gtk::Button>()
                .unwrap()
                .emit_clicked();
            }
            14 => {
                match library_toolbar_stage.get() {
                    0 => {
                        screenshot(main.upcast_ref(), "library-all");
                        main.set_default_size(900, 600);
                    }
                    1 => {
                        screenshot(main.upcast_ref(), "library-narrow");
                        find(root, |w| {
                            w.has_css_class("media-view-switch") && w.is_mapped()
                        })
                        .downcast::<adw::ToggleGroup>()
                        .unwrap()
                        .set_active_name(Some("list"));
                    }
                    2 => {
                        screenshot(main.upcast_ref(), "library-list");
                        find(root, |w| {
                            w.has_css_class("media-sort-order") && w.is_mapped()
                        })
                        .downcast::<adw::ToggleGroup>()
                        .unwrap()
                        .set_active_name(Some("desc"));
                    }
                    3 => {
                        screenshot(main.upcast_ref(), "library-sort-descending");
                        find(root, |w| {
                            w.has_css_class("media-view-switch") && w.is_mapped()
                        })
                        .downcast::<adw::ToggleGroup>()
                        .unwrap()
                        .set_active_name(Some("grid"));
                        main.set_default_size(1152, 720);
                    }
                    4 => library_tab(root, "resume"),
                    _ => unreachable!(),
                }
                library_toolbar_stage.set(library_toolbar_stage.get() + 1);
                if library_toolbar_stage.get() <= 4 {
                    return glib::ControlFlow::Continue;
                }
            }
            15 => {
                screenshot(main.upcast_ref(), "library-resume");
                library_tab(root, "genres");
            }
            16 => {
                screenshot(main.upcast_ref(), "library-genres");
                library_tab(root, "liked");
            }
            17 => {
                screenshot(main.upcast_ref(), "library-liked");
                find(root, |w| {
                    w.has_css_class("media-filter-button") && w.is_mapped()
                })
                .downcast::<gtk::Button>()
                .unwrap()
                .emit_clicked();
            }
            18 => {
                screenshot(main.upcast_ref(), "filter-dialog");
                let row = find(root, |w| w.type_().name() == "FiltersRow" && w.is_mapped());
                find(&row, |w| w.is::<gtk::Button>())
                    .downcast::<gtk::Button>()
                    .unwrap()
                    .emit_clicked();
            }
            19 => {
                screenshot(main.upcast_ref(), "filter-search");
                find(root, |w| {
                    w.is::<adw::NavigationView>()
                        && w.is_mapped()
                        && w.ancestor(adw::Dialog::static_type()).is_some()
                })
                .downcast::<adw::NavigationView>()
                .unwrap()
                .pop();
                dialog(root).close();
                main.homepage();
            }
            20 => {
                click_card(root, "Movie");
            }
            21 => {
                screenshot(main.upcast_ref(), "movie");
                main.set_default_size(900, 600);
            }
            22 => {
                screenshot(main.upcast_ref(), "movie-narrow");
                find(root, |w| {
                    w.type_().name() == "ItemActionsBox" && w.is_mapped()
                })
                .activate_action("item.editm", None)
                .unwrap();
            }
            23 => {
                screenshot(main.upcast_ref(), "metadata-dialog");
                dialog(root).close();
                find(root, |w| {
                    w.type_().name() == "ItemActionsBox" && w.is_mapped()
                })
                .activate_action("item.editi", None)
                .unwrap();
            }
            24 => {
                screenshot(main.upcast_ref(), "images-dialog");
                dialog(root).close();
            }
            25 => {
                main.set_default_size(1152, 720);
                main.homepage();
            }
            26 => {
                card_action(root, "Movie", "item.refresh");
            }
            27 => {
                screenshot(main.upcast_ref(), "refresh-dialog");
                dialog(root).close();
            }
            28 => {
                card_action(root, "Movie", "item.identify");
            }
            29 => {
                screenshot(main.upcast_ref(), "identify-dialog");
                dialog(root).close();
            }
            30 => {
                card_action(root, "Series", "item.view-missing");
            }
            31 => {
                screenshot(main.upcast_ref(), "missing-episodes");
                dialog(root).close();
            }
            32 => {
                click_card(root, "Series");
            }
            33 => {
                screenshot(main.upcast_ref(), "series");
                main.homepage();
            }
            34 => {
                click_card(root, "MusicAlbum");
            }
            35 => {
                screenshot(main.upcast_ref(), "album");
                main.homepage();
            }
            36 => {
                click_card(root, "Person");
            }
            37 => {
                screenshot(main.upcast_ref(), "person");
                main.homepage();
            }
            38 => {
                find(root, |w| w.has_css_class("window-stack"))
                    .downcast::<gtk::Stack>()
                    .unwrap()
                    .set_visible_child_name("mpv");
                for revealer in descendants(root)
                    .into_iter()
                    .filter_map(|widget| widget.downcast::<gtk::Revealer>().ok())
                {
                    if revealer.child().is_some_and(|child| {
                        child.has_css_class("mpv-top-bar") || child.has_css_class("mpv-bottom-bar")
                    }) {
                        revealer.set_reveal_child(true);
                    }
                }
                root.activate_action("win.mpv-settings", None).unwrap();
            }
            39 => {
                screenshot(main.upcast_ref(), "player-controls");
                screenshot(main.upcast_ref(), "player-settings");
                player_tab(root, "win.mpv-settings");
                let nav = find(root, |w| {
                    w.has_css_class("mpv-sidebar-switcher") && w.is_mapped()
                });
                assert_eq!(
                    main_navigation.borrow().as_ref().unwrap().radii,
                    surface(&nav).radii,
                    "Main/player navigation geometry"
                );
                root.activate_action("win.mpv-shortcuts", None).unwrap();
            }
            40 => {
                screenshot(main.upcast_ref(), "player-shortcuts");
                player_tab(root, "win.mpv-shortcuts");
                root.activate_action("win.mpv-media-info", None).unwrap();
            }
            41 => {
                screenshot(main.upcast_ref(), "player-info");
                player_tab(root, "win.mpv-media-info");
                main.set_default_size(900, 600);
            }
            42 => {
                screenshot(main.upcast_ref(), "player-narrow");
                player_tab(root, "win.mpv-media-info");
                root.activate_action("win.mpv-settings", None).unwrap();
                switch_theme(if adw::StyleManager::default().is_dark() {
                    2
                } else {
                    3
                });
            }
            43 => {
                screenshot(main.upcast_ref(), "player-switched-theme");
                player_tab(root, "win.mpv-settings");
                switch_theme(
                    std::env::var("AUDIT_THEME")
                        .unwrap_or_else(|_| "3".into())
                        .parse()
                        .unwrap(),
                );
                find(root, |w| w.has_css_class("window-stack"))
                    .downcast::<gtk::Stack>()
                    .unwrap()
                    .set_visible_child_name("main");
                main.set_default_size(1152, 720);
                main.homepage();
                root.activate_action("win.add-server", None).unwrap();
            }
            44 => {
                screenshot(main.upcast_ref(), "add-server");
                dialog(root).close();
                root.activate_action("win.server-panel", None).unwrap();
            }
            45 => {
                screenshot(main.upcast_ref(), "server-panel");
                main.homepage();
                root.activate_action("win.settings", None).unwrap();
            }
            46 => {
                let settings = settings_window();
                find(settings.upcast_ref(), |w| {
                    w.is::<adw::ActionRow>()
                        && w.property::<String>("title") == "Preferred Video Version"
                })
                .emit_by_name::<()>("activated", &[]);
            }
            47 => {
                let settings = settings_window();
                screenshot(settings.upcast_ref(), "settings-versions");
                find(settings.upcast_ref(), |w| {
                    w.downcast_ref::<gtk::Button>()
                        .is_some_and(|b| b.icon_name().as_deref() == Some("list-add-symbolic"))
                        && w.is_mapped()
                })
                .downcast::<gtk::Button>()
                .unwrap()
                .emit_clicked();
            }
            48 => {
                let settings = settings_window();
                screenshot(settings.upcast_ref(), "settings-version-editor");
                dialog(settings.upcast_ref()).close();
                settings.close();
                click_card(root, "Movie");
            }
            49 => {
                find(root, |w| {
                    w.type_().name() == "ItemActionsBox" && w.is_mapped()
                })
                .activate_action("item.editi", None)
                .unwrap();
            }
            50 => {
                find(root, |w| {
                    w.type_().name() == "ImageInfoCard"
                        && w.property::<bool>("searchable")
                        && w.is_mapped()
                })
                .activate_action("image.edit", None)
                .unwrap();
            }
            51 => {
                screenshot(main.upcast_ref(), "image-editor");
                find(root, |w| {
                    w.is::<adw::NavigationView>()
                        && w.is_mapped()
                        && w.ancestor(adw::Dialog::static_type()).is_some()
                })
                .downcast::<adw::NavigationView>()
                .unwrap()
                .pop();
            }
            52 => {
                find(root, |w| {
                    w.type_().name() == "ImageInfoCard"
                        && w.property::<bool>("searchable")
                        && w.is_mapped()
                })
                .activate_action("image.search", None)
                .unwrap();
            }
            53 => {
                screenshot(main.upcast_ref(), "image-search");
                dialog(root).close();
                root.activate_action("win.settings", None).unwrap();
            }
            54..=63 => {
                let settings = settings_window();
                let pages = ["General", "Account", "media", "Others", "Shortcuts"];
                let page = pages[((stage.get() - 54) / 2) as usize];
                if stage.get() % 2 == 0 {
                    settings.set_visible_page_name(page);
                    glib::timeout_add_local_once(Duration::from_millis(80), move || {
                        scroll_to_bottom(settings.upcast_ref());
                    });
                } else {
                    screenshot(settings.upcast_ref(), &format!("settings-{page}-bottom"));
                    if page == "Account" {
                        let row = find(settings.upcast_ref(), |w| {
                            w.is::<adw::ButtonRow>()
                                && w.has_css_class("suggested-action")
                                && w.is_mapped()
                        });
                        row.set_sensitive(false);
                        glib::timeout_add_local_once(Duration::from_millis(100), move || {
                            screenshot(settings.upcast_ref(), "settings-disabled-action");
                            row.set_sensitive(true);
                            row.set_state_flags(gtk::StateFlags::PRELIGHT, false);
                            glib::timeout_add_local_once(Duration::from_millis(100), move || {
                                screenshot(settings.upcast_ref(), "settings-action-hover");
                                row.unset_state_flags(gtk::StateFlags::PRELIGHT);
                            });
                        });
                    }
                }
            }
            64 => {
                settings_window().close();
                main.set_default_size(900, 600);
                find(root, |w| w.has_css_class("window-stack"))
                    .downcast::<gtk::Stack>()
                    .unwrap()
                    .set_visible_child_name("mpv");
                root.activate_action("win.mpv-shortcuts", None).unwrap();
                root.activate_action("win.mpv-settings", None).unwrap();
                let root = root.clone();
                glib::timeout_add_local_once(Duration::from_millis(80), move || {
                    scroll_to_bottom(&root);
                });
            }
            65 => {
                screenshot(main.upcast_ref(), "player-settings-bottom");
                player_tab(root, "win.mpv-settings");
                main.application().unwrap().quit();
                MISSING_ICONS.with(|icons| {
                    assert!(
                        icons.borrow().is_empty(),
                        "Missing icons: {:?}",
                        icons.borrow()
                    )
                });
                println!(
                    "PASS native UI audit; fixture screenshots: {}",
                    output_dir().display()
                );
                return glib::ControlFlow::Break;
            }
            _ => unreachable!(),
        }
        stage.set(stage.get() + 1);
        glib::ControlFlow::Continue
    });
    tsukimi::run();
}
