use super::{
    episode_switcher::EpisodeButton,
    fix::ScrolledWindowFixExt,
    hortu_scrolled::{SHOW_BUTTON_ANIMATION_DURATION, UnifySize},
    item_utils::*,
    song_widget::format_duration,
    utils::{GlobalToast, run_time_ticks_to_label},
    window::Window,
};
use crate::{
    client::{error::UserFacingError, jellyfin_client::JELLYFIN_CLIENT, structs::*},
    ui::{
        mpv::page::{PlaybackDirectMode, media_source_stream_url},
        provider::{
            dropdown_factory::{DropdownList, DropdownListBuilder},
            tu_item::TuItem,
            tu_object::TuObject,
        },
    },
    utils::{
        CacheEvent, CachePolicy, fetch_with_cache, get_image_with_cache, spawn, spawn_g_timeout,
        spawn_tokio,
    },
};
use adw::{prelude::*, subclass::prelude::*};
use chrono::{DateTime, Utc};
use gettextrs::gettext;
use glib::Object;
use gtk::{ListScrollFlags, ListView, gio, glib, template_callbacks};

pub(crate) mod imp {
    use std::cell::{OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use glib::subclass::InitializingObject;
    use gtk::{CompositeTemplate, glib, prelude::*};

    use super::SimpleListItem;
    use crate::{
        ui::{
            provider::{dropdown_factory::factory, tu_item::TuItem, tu_object::TuObject},
            widgets::{
                EpisodeSwitcher, fix::ScrolledWindowFixExt, horbu_scrolled::HorbuScrolled,
                hortu_scrolled::HortuScrolled, item_actionbox::ItemActionsBox,
                item_carousel::ItemCarousel, star_toggle::StarToggle,
                tu_overview_item::imp::ViewGroup, utils::TuItemBuildExt,
            },
        },
        utils::spawn,
    };

    // Object holding the state
    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[template(resource = "/moe/tsuna/tsukimi/ui/item.ui")]
    #[properties(wrapper_type = super::ItemPage)]
    pub struct ItemPage {
        #[property(get, set, construct_only)]
        pub item: OnceCell<TuItem>,

        #[template_child]
        pub actorhortu: TemplateChild<HortuScrolled>,
        #[template_child]
        pub recommendhortu: TemplateChild<HortuScrolled>,
        #[template_child]
        pub includehortu: TemplateChild<HortuScrolled>,
        #[template_child]
        pub additionalhortu: TemplateChild<HortuScrolled>,
        #[template_child]
        pub seasonshortu: TemplateChild<HortuScrolled>,

        #[template_child]
        pub studioshorbu: TemplateChild<HorbuScrolled>,
        #[template_child]
        pub tagshorbu: TemplateChild<HorbuScrolled>,
        #[template_child]
        pub genreshorbu: TemplateChild<HorbuScrolled>,
        #[template_child]
        pub linkshorbu: TemplateChild<HorbuScrolled>,

        #[template_child]
        pub itemlist: TemplateChild<gtk::ListView>,
        #[template_child]
        pub logobox: TemplateChild<gtk::Box>,
        #[template_child]
        pub seasonlist: TemplateChild<gtk::DropDown>,

        #[template_child]
        pub mediainfobox: TemplateChild<gtk::Box>,
        #[template_child]
        pub mediainforevealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub detail_scrolled: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub scrolled: TemplateChild<gtk::ScrolledWindow>,

        #[template_child]
        pub line1: TemplateChild<gtk::Label>,
        #[template_child]
        pub episode_line: TemplateChild<gtk::Label>,
        #[template_child]
        pub line2: TemplateChild<gtk::Label>,
        #[template_child]
        pub crating: TemplateChild<gtk::Label>,
        #[template_child]
        pub orating: TemplateChild<gtk::Label>,
        #[template_child]
        pub star: TemplateChild<gtk::Image>,

        #[template_child]
        pub playbutton: TemplateChild<gtk::Button>,
        #[template_child]
        pub namedropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub subdropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub carousel: TemplateChild<ItemCarousel>,
        #[template_child]
        pub actionbox: TemplateChild<ItemActionsBox>,
        #[template_child]
        pub tagline: TemplateChild<gtk::Label>,
        #[template_child]
        pub toolbar: TemplateChild<gtk::Box>,
        #[template_child]
        pub episode_list_bin: TemplateChild<adw::Bin>,

        #[template_child]
        pub spinner: TemplateChild<adw::Spinner>,

        #[template_child]
        pub buttoncontent: TemplateChild<adw::ButtonContent>,

        #[template_child]
        pub indicator: TemplateChild<adw::CarouselIndicatorDots>,

        pub selection: gtk::SingleSelection,
        pub seasonselection: gtk::SingleSelection,
        pub playbuttonhandlerid: RefCell<Option<glib::SignalHandlerId>>,

        #[property(get, set, construct_only)]
        pub name: RefCell<Option<String>>,
        pub selected: RefCell<Option<String>>,

        pub videoselection: gtk::SingleSelection,
        pub subselection: gtk::SingleSelection,

        #[template_child]
        pub left_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub right_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub episode_stack: TemplateChild<gtk::Stack>,

        #[template_child]
        pub episode_switcher: TemplateChild<EpisodeSwitcher>,

        pub show_button_animation: OnceCell<adw::TimedAnimation>,
        pub hide_button_animation: OnceCell<adw::TimedAnimation>,

        pub season_id: RefCell<Option<String>>,

        #[property(get, set, nullable)]
        pub current_item: RefCell<Option<TuItem>>,

        // None if season not changed
        #[property(get, set, nullable)]
        pub current_season: RefCell<Option<String>>,
        #[property(get, set, nullable)]
        pub play_session_id: RefCell<Option<String>>,

        pub season_list_vec: RefCell<Vec<SimpleListItem>>,

        pub episode_list_vec: RefCell<Vec<SimpleListItem>>,

        pub video_version_matcher: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ItemPage {
        const NAME: &'static str = "ItemPage";
        type Type = super::ItemPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            ItemCarousel::ensure_type();
            StarToggle::ensure_type();
            HortuScrolled::ensure_type();
            HorbuScrolled::ensure_type();
            EpisodeSwitcher::ensure_type();
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ItemPage {
        fn constructed(&self) {
            self.parent_constructed();
            self.scrolled.fix();

            self.indicator
                .set_carousel(Some(&self.carousel.imp().carousel));

            let namedropdown = self.namedropdown.get();
            let subdropdown = self.subdropdown.get();
            namedropdown.set_factory(Some(&factory::<true>()));
            namedropdown.set_list_factory(Some(&factory::<false>()));
            subdropdown.set_factory(Some(&factory::<true>()));
            subdropdown.set_list_factory(Some(&factory::<false>()));

            let store = gtk::gio::ListStore::new::<TuObject>();
            self.selection.set_model(Some(&store));
            self.itemlist.set_model(Some(&self.selection));
            self.itemlist.set_factory(Some(
                gtk::SignalListItemFactory::new().tu_overview_item(ViewGroup::EpisodesView),
            ));

            let item = self.obj().item();

            if item.item_type() == "Series"
                || (item.item_type() == "Episode" && item.series_name().is_some())
            {
                self.toolbar.set_visible(true);
                self.episode_list_bin.set_visible(true);
                self.episode_line.set_visible(true);
            }

            let obj = self.obj();
            spawn(glib::clone!(
                #[weak]
                obj,
                async move {
                    obj.setup().await;
                }
            ));
        }
    }

    impl WidgetImpl for ItemPage {}

    impl WindowImpl for ItemPage {}

    impl ApplicationWindowImpl for ItemPage {}

    impl adw::subclass::navigation_page::NavigationPageImpl for ItemPage {}
}

glib::wrapper! {
    pub struct ItemPage(ObjectSubclass<imp::ItemPage>)
        @extends gtk::ApplicationWindow, gtk::Window, gtk::Widget ,adw::NavigationPage,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

#[template_callbacks]
impl ItemPage {
    pub fn new(item: &TuItem) -> Self {
        Object::builder().property("item", item).build()
    }

    pub async fn setup(&self) {
        self.reset_detail_scroll_to_top();

        let item = self.item();
        let type_ = item.item_type();
        let imp = self.imp();

        if let Some(series_name) = item.series_name() {
            imp.line1.set_text(&series_name);
        } else {
            imp.line1.set_text(&item.name());
        }

        if type_ == "Series" {
            let series_id = item.id();

            spawn(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[strong]
                series_id,
                async move {
                    let Some(intro) = obj.set_first_episode(&series_id).await else {
                        obj.imp()
                            .buttoncontent
                            .set_label(&gettext("Select an episode"));
                        return;
                    };
                    obj.set_intro::<false>(&intro).await;
                }
            ));

            self.imp().actionbox.set_id(Some(series_id.to_owned()));
            self.setup_item(&series_id).await;
            self.setup_seasons(&series_id).await;
        } else if type_ == "Episode" && item.series_name().is_some() {
            let series_id = item.series_id().unwrap_or(item.id());

            spawn(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[weak]
                item,
                async move {
                    obj.set_intro::<false>(&item).await;
                }
            ));

            self.imp().actionbox.set_id(Some(series_id.to_owned()));
            self.setup_item(&series_id).await;
            self.setup_seasons(&series_id).await;
        } else {
            let id = item.id();

            spawn(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    obj.set_intro::<true>(&item).await;
                }
            ));

            self.imp().actionbox.set_id(Some(id.to_owned()));
            self.setup_item(&id).await;
        }
    }

    pub async fn update_intro(&self) {
        let item = self.item();

        if item.item_type() == "Series" {
            let series_id = item.series_id().unwrap_or(item.id());

            spawn(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[strong]
                series_id,
                async move {
                    let Some(intro) = obj.set_first_episode(&series_id).await else {
                        return;
                    };
                    obj.set_intro::<false>(&intro).await;
                }
            ));
        }

        if item.item_type() == "Video" || item.item_type() == "Movie" {
            spawn(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[weak]
                item,
                async move {
                    let id = item.id();
                    match spawn_tokio(async move { JELLYFIN_CLIENT.get_item_info(&id).await }).await
                    {
                        Ok(item) => {
                            obj.set_intro::<true>(&TuItem::from_simple(item)).await;
                        }
                        Err(e) => {
                            obj.toast(e.to_user_facing());
                        }
                    }
                }
            ));
        }
    }

    async fn setup_item(&self, id: &str) {
        self.reset_detail_scroll_to_top();

        let id = id.to_string();
        let id_clone = id.to_owned();

        spawn(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                obj.set_logo(&id_clone).await;
            }
        ));

        self.setup_background(&id).await;
        self.set_overview(&id).await;
        self.set_lists(&id).await;
    }

    fn reset_detail_scroll_to_top(&self) {
        let scrolled = self.imp().detail_scrolled.get();
        let adj = scrolled.vadjustment();
        adj.set_value(adj.lower());

        glib::idle_add_local_once(move || {
            let adj = scrolled.vadjustment();
            adj.set_value(adj.lower());
        });
    }

    async fn set_intro<const IS_VIDEO: bool>(&self, intro: &TuItem) {
        let intro_id = intro.id();
        let play_button = self.imp().playbutton.get();
        let spinner = self.imp().spinner.get();

        self.set_now_item::<IS_VIDEO>(intro);

        play_button.set_sensitive(false);
        spinner.set_visible(true);

        let intro_id_clone = intro_id.to_owned();
        let playback = match spawn_tokio(async move {
            JELLYFIN_CLIENT
                .get_playbackinfo(
                    &intro_id_clone,
                    None,
                    None,
                    false,
                    PlaybackDirectMode::direct(),
                )
                .await
        })
        .await
        {
            Ok(playback) => playback,
            Err(e) => {
                self.toast(e.to_user_facing());
                return;
            }
        };

        self.set_current_item(Some(intro));
        self.set_dropdown(&playback).await;
        self.set_play_session_id(playback.play_session_id.to_owned());

        play_button.set_sensitive(true);
        spinner.set_visible(false);

        self.createmediabox(playback.media_sources, None).await;
    }

    #[template_callback]
    async fn on_season_selected(&self, _param: Option<glib::ParamSpec>, dropdown: gtk::DropDown) {
        let item = self.item();

        let item_type = item.item_type();

        if item_type != "Series" && item_type != "Episode" {
            return;
        }

        let object = dropdown.selected_item();
        let Some(season_name) = object.and_downcast_ref::<gtk::StringObject>() else {
            return;
        };

        let season_name = season_name.string().to_string();

        let imp = self.imp();
        imp.episode_stack.set_visible_child_name("loading");

        let series_id = item.series_id().unwrap_or(item.id());

        let position = dropdown.selected();
        let season_id = item.season_id();

        let list = match (position, season_id.to_owned()) {
            (0, Some(season_id)) => {
                let season_id_clone = season_id.to_owned();
                match spawn_tokio(async move {
                    JELLYFIN_CLIENT
                        .get_episodes(&series_id, &season_id.to_string(), 0)
                        .await
                })
                .await
                {
                    Ok(item) => {
                        self.set_current_season(Some(season_id_clone));
                        item
                    }
                    Err(e) => {
                        self.toast(e.to_user_facing());
                        return;
                    }
                }
            }
            (0, None) => {
                match spawn_tokio(async move {
                    JELLYFIN_CLIENT.get_continue_play_list(&series_id).await
                })
                .await
                {
                    Ok(item) => item,
                    Err(e) => {
                        self.toast(e.to_user_facing());
                        return;
                    }
                }
            }
            _ => {
                let season_id = {
                    let season_list = imp.season_list_vec.borrow();
                    let Some(season) = season_list.iter().find(|s| s.name == season_name) else {
                        return;
                    };
                    self.imp().season_id.replace(Some(season.id.to_owned()));
                    season.id.to_owned()
                };

                match spawn_tokio(async move {
                    JELLYFIN_CLIENT
                        .get_episodes(&series_id, &season_id, 0)
                        .await
                })
                .await
                {
                    Ok(list) => list,
                    Err(e) => {
                        self.toast(e.to_user_facing());
                        return;
                    }
                }
            }
        };

        let index = list
            .items
            .iter()
            .position(|item| item.index_number == Some(self.item().index_number()))
            .unwrap_or(0);

        self.set_episode_list(list.items);

        if position == 0 {
            // itemlist need wait for property binding to scroll
            spawn_g_timeout(glib::clone!(
                #[weak]
                imp,
                async move {
                    imp.itemlist
                        .scroll_to(index as u32, ListScrollFlags::all(), None);
                },
            ));
        } else {
            self.imp().episode_switcher.load_from_n_items(
                list.total_record_count as usize,
                glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |btn| {
                        spawn(glib::clone!(
                            #[weak]
                            obj,
                            #[weak]
                            btn,
                            async move {
                                obj.on_episode_switcher_clicked(&btn).await;
                            }
                        ))
                    }
                ),
            );
        }
    }

    fn set_episode_list(&self, list: Vec<SimpleListItem>) {
        let imp = self.imp();
        let store_model = imp.selection.model();
        let Some(store) = store_model.and_downcast_ref::<gio::ListStore>() else {
            return;
        };

        store.remove_all();

        if list.is_empty() {
            imp.episode_stack.set_visible_child_name("fallback");
            return;
        }

        let items = list
            .iter()
            .map(|item| TuObject::from_simple(item.to_owned()))
            .collect::<Vec<_>>();

        store.extend_from_slice(&items);

        imp.episode_list_vec.replace(list);
        imp.episode_stack.set_visible_child_name("view");
    }

    async fn on_episode_switcher_clicked(&self, btn: &EpisodeButton) {
        let imp = self.imp();

        let start_index = btn.start_index();
        let item = self.item();
        let series_id = item.series_id().unwrap_or(item.id());

        let Some(season_id) =
            self.current_season().or(item
                .season_id()
                .or(self.imp().season_id.borrow().to_owned()))
        else {
            return;
        };

        imp.episode_stack.set_visible_child_name("loading");

        let list = match spawn_tokio(async move {
            JELLYFIN_CLIENT
                .get_episodes(&series_id, &season_id, start_index)
                .await
        })
        .await
        {
            Ok(list) => list,
            Err(e) => {
                self.toast(e.to_user_facing());
                return;
            }
        };

        self.set_episode_list(list.items);
    }

    async fn set_first_episode(&self, id: &str) -> Option<TuItem> {
        let id = id.to_string();
        let seasons_id = id.clone();
        let mut seasons =
            match spawn_tokio(async move { JELLYFIN_CLIENT.get_season_list(&seasons_id).await })
                .await
            {
                Ok(seasons) => seasons.items,
                Err(error) => {
                    self.toast(error.to_user_facing());
                    return None;
                }
            };
        // Prefer the first numbered season. Specials (season 0) are only used
        // when the series has no regular season.
        seasons.sort_by_key(|season| {
            (
                season.index_number.unwrap_or(u32::MAX) == 0,
                season.index_number.unwrap_or(u32::MAX),
            )
        });

        for season in seasons {
            let series_id = id.clone();
            let season_id = season.id;
            let episodes = match spawn_tokio(async move {
                JELLYFIN_CLIENT
                    .get_episodes(&series_id, &season_id, 0)
                    .await
            })
            .await
            {
                Ok(episodes) => episodes,
                Err(error) => {
                    tracing::warn!(
                        series_id = %id,
                        %error,
                        "Unable to load episodes while selecting the first episode"
                    );
                    continue;
                }
            };

            if let Some(first_episode) = episodes.items.into_iter().min_by_key(|episode| {
                (
                    episode.parent_index_number.unwrap_or(u32::MAX),
                    episode.index_number.unwrap_or(u32::MAX),
                )
            }) {
                let tu_item = TuItem::from_simple(first_episode);
                self.set_now_item::<false>(&tu_item);
                return Some(tu_item);
            }
        }

        None
    }

    fn set_now_item<const IS_VIDEO: bool>(&self, item: &TuItem) {
        let imp = self.imp();

        if IS_VIDEO {
            imp.episode_line.set_text(&item.name());
        } else {
            imp.episode_line.set_text(&format!(
                "S{}E{}: {}",
                item.parent_index_number(),
                item.index_number(),
                item.name()
            ));
        }

        let sec = item.playback_position_ticks() / 10000000;
        if sec > 10 {
            imp.buttoncontent.set_label(&format!(
                "{} {}",
                gettext("Resume"),
                format_duration(sec as i64)
            ));
        } else {
            imp.buttoncontent.set_label(&gettext("Play"));
        }
    }

    pub async fn set_dropdown(&self, playbackinfo: &Media) {
        let imp = self.imp();
        let namedropdown = imp.namedropdown.get();
        let subdropdown = imp.subdropdown.get();

        let matcher = imp.video_version_matcher.borrow().to_owned();

        let vstore = gtk::gio::ListStore::new::<glib::BoxedAnyObject>();
        imp.videoselection.set_model(Some(&vstore));

        let sstore = gtk::gio::ListStore::new::<glib::BoxedAnyObject>();
        imp.subselection.set_model(Some(&sstore));

        namedropdown.set_model(Some(&imp.videoselection));
        subdropdown.set_model(Some(&imp.subselection));

        let media_sources = playbackinfo.media_sources.to_owned();

        let mut v_dl: Vec<String> = Vec::new();

        namedropdown.connect_selected_item_notify(glib::clone!(
            #[weak]
            imp,
            move |dropdown| {
                let Some(entry) = dropdown
                    .selected_item()
                    .and_downcast::<glib::BoxedAnyObject>()
                else {
                    return;
                };

                let dl: std::cell::Ref<DropdownList> = entry.borrow();
                let selected = &dl.id;
                for _i in 0..sstore.n_items() {
                    sstore.remove(0);
                }
                for media in &media_sources {
                    if selected.as_deref().is_some_and(|s| s == media.id) {
                        let mut lang_list = Vec::new();
                        for stream in &media.media_streams {
                            if stream.stream_type == "Subtitle" {
                                let Ok(dl) = DropdownListBuilder::default()
                                    .line1(stream.display_title.to_owned())
                                    .line2(stream.title.to_owned())
                                    .sub_lang(stream.language.to_owned())
                                    .index(Some(stream.index))
                                    .url(stream.delivery_url.to_owned())
                                    .is_external(Some(stream.is_external))
                                    .build()
                                else {
                                    continue;
                                };

                                let match_text = [
                                    stream.display_title.as_deref(),
                                    stream.title.as_deref(),
                                    stream.language.as_deref(),
                                    stream.display_language.as_deref(),
                                ]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(" ");
                                lang_list.push((stream.index, match_text));
                                let object = glib::BoxedAnyObject::new(dl);
                                sstore.append(&object);
                            }
                        }

                        if let Some(u) = make_subtitle_version_choice(lang_list) {
                            subdropdown.set_selected(u.1 as u32);
                        }
                        break;
                    }
                }

                imp.video_version_matcher.replace(dl.line1.to_owned());
            }
        ));

        for (index, media) in playbackinfo.media_sources.iter().enumerate() {
            let line2 = media
                .bit_rate
                .map(|bit_rate| format!("{:.2} Kbps", bit_rate as f64 / 1_000.0))
                .unwrap_or_default();
            let play_url = media_source_stream_url(media).await;
            let Ok(dl) = DropdownListBuilder::default()
                .line1(Some(media.name.to_owned()))
                .line2(Some(line2))
                .url(play_url)
                .id(Some(media.id.to_owned()))
                .index(Some(index as i64))
                .build()
            else {
                continue;
            };

            v_dl.push(dl.line1.to_owned().unwrap_or_default());
            let object = glib::BoxedAnyObject::new(dl);
            vstore.append(&object);
        }

        if let Some(matcher) = matcher {
            if let Some(p) = make_video_version_choice_from_matcher(v_dl, &matcher) {
                namedropdown.set_selected(p as u32);
            }
        } else if let Some(p) = make_video_version_choice_from_filter(v_dl) {
            namedropdown.set_selected(p as u32);
        }
    }

    pub async fn setup_background(&self, id: &str) {
        let imp = self.imp();

        let backdrop = imp.carousel.imp().backdrop.get();
        let path = get_image_with_cache(id.to_string(), "Backdrop".to_string(), Some(0))
            .await
            .unwrap_or_default();
        let file = gtk::gio::File::for_path(&path);
        backdrop.set_file(Some(&file));
        self.imp()
            .carousel
            .imp()
            .backrevealer
            .set_reveal_child(true);
    }

    pub async fn add_backdrops(&self, image_tags: Vec<String>, id: &str) {
        let imp = self.imp();
        let tags = image_tags.len();
        let carousel = imp.carousel.imp().carousel.get();
        for tag_num in 1..tags {
            let path =
                get_image_with_cache(id.to_string(), "Backdrop".to_string(), Some(tag_num as u8))
                    .await
                    .unwrap_or_default();
            let file = gtk::gio::File::for_path(&path);
            let picture = gtk::Picture::builder()
                .halign(gtk::Align::Fill)
                .valign(gtk::Align::Fill)
                .content_fit(gtk::ContentFit::Cover)
                .file(&file)
                .build();
            carousel.append(&picture);
        }
    }

    pub async fn setup_seasons(&self, id: &str) {
        let imp = self.imp();
        let id = id.to_string();

        let Some(season_list_store) = imp.seasonlist.model().and_downcast::<gtk::StringList>()
        else {
            return;
        };

        let mut events = fetch_with_cache(
            &format!("season_{}", id),
            CachePolicy::ReadCacheAndRefresh,
            async move { JELLYFIN_CLIENT.get_season_list(&id).await },
        )
        .await;

        while let Some(event) = events.recv().await {
            match event {
                CacheEvent::Data { data, .. } => {
                    let season_list = data.items;
                    let names = season_list
                        .iter()
                        .map(|season| season.name.as_str())
                        .collect::<Vec<_>>();
                    season_list_store.splice(
                        1,
                        season_list_store.n_items().saturating_sub(1),
                        &names,
                    );
                    imp.seasonshortu.set_items(season_list.to_owned());
                    let first_season = if self.item().item_type() == "Series" {
                        season_list
                            .iter()
                            .enumerate()
                            .filter(|(_, season)| season.index_number.unwrap_or(0) > 0)
                            .min_by_key(|(_, season)| season.index_number.unwrap_or(u32::MAX))
                            .map(|(index, _)| index + 1)
                            .unwrap_or(1)
                            .min(season_list.len())
                    } else {
                        // Opening an individual episode keeps the legacy
                        // current-season selection and scroll behavior.
                        0
                    };
                    imp.season_list_vec.replace(season_list);
                    let first_season = first_season as u32;
                    if imp.seasonlist.selected() == first_season {
                        self.on_season_selected(None, imp.seasonlist.get()).await;
                    } else {
                        imp.seasonlist.set_selected(first_season);
                    }
                }
                CacheEvent::Error(e) => {
                    self.toast(e.to_user_facing());
                    return;
                }
            }
        }
    }

    #[template_callback]
    async fn on_item_activated(&self, position: u32, view: &ListView) {
        let Some(model) = view.model() else {
            return;
        };
        let Some(item) = model.item(position).and_downcast::<TuObject>() else {
            return;
        };
        self.set_intro::<false>(&item.item()).await;
    }

    pub async fn set_logo(&self, id: &str) {
        let logo = super::logo::set_logo(id.to_string(), "Logo", None).await;
        self.imp().logobox.append(&logo);
    }

    pub async fn set_overview(&self, id: &str) {
        let id = id.to_string();

        let mut events = fetch_with_cache(
            &format!("item_{}", id),
            CachePolicy::ReadCacheAndRefresh,
            async move { JELLYFIN_CLIENT.get_item_info(&id).await },
        )
        .await;

        while let Some(event) = events.recv().await {
            match event {
                CacheEvent::Data { data: item, .. } => spawn(glib::clone!(
                    #[weak(rename_to = obj)]
                    self,
                    async move {
                        {
                            let mut str = String::new();
                            if let Some(communityrating) = item.community_rating {
                                let formatted_rating = format!("{communityrating:.1}");
                                let crating = obj.imp().crating.get();
                                crating.set_text(&formatted_rating);
                                crating.set_visible(true);
                                obj.imp().star.get().set_visible(true);
                            }
                            if let Some(rating) = item.official_rating {
                                let orating = obj.imp().orating.get();
                                orating.set_text(&rating);
                                orating.set_visible(true);
                            }
                            if let Some(year) = item.production_year {
                                str.push_str(&year.to_string());
                                str.push_str("  ");
                            }
                            if let Some(runtime) = item.run_time_ticks {
                                let time_string = run_time_ticks_to_label(runtime);
                                str.push_str(&time_string);
                                str.push_str("  ");
                            }
                            if let Some(genres) = &item.genres {
                                for genre in genres {
                                    str.push_str(&genre.name);
                                    str.push(',');
                                }
                                str.pop();
                            }
                            obj.imp().line2.get().set_text(&str);

                            if let Some(taglines) = item.taglines
                                && let Some(tagline) = taglines.first()
                            {
                                obj.imp().tagline.set_text(tagline);
                                obj.imp().tagline.set_visible(true);
                            }
                        }
                        if let Some(links) = item.external_urls {
                            obj.set_flowlinks(links);
                        }
                        if let Some(actor) = item.people {
                            obj.setactorscrolled(actor).await;
                        }
                        if let Some(studios) = item.studios {
                            obj.set_flowbuttons(studios, "Studios");
                        }
                        if let Some(tags) = item.tags {
                            obj.set_flowbuttons(tags, "Tags");
                        }
                        if let Some(genres) = item.genres {
                            obj.set_flowbuttons(genres, "Genres");
                        }
                        if let Some(image_tags) = item.backdrop_image_tags {
                            obj.add_backdrops(image_tags, &item.id).await;
                        }
                        if let Some(part_count) = item.part_count
                            && part_count > 1
                        {
                            obj.sets("Additional Parts", &item.id).await;
                        }
                        if let Some(ref user_data) = item.user_data {
                            let imp = obj.imp();
                            if let Some(is_favourite) = user_data.is_favorite {
                                imp.actionbox.set_btn_active(is_favourite);
                            }
                            imp.actionbox.set_played(user_data.played);
                            imp.actionbox.bind_edit();
                        }
                    }
                )),
                CacheEvent::Error(e) => {
                    self.toast(e.to_user_facing());
                    return;
                }
            }
        }
    }

    pub async fn createmediabox(
        &self, media_sources: Vec<MediaSource>, date_created: Option<DateTime<Utc>>,
    ) {
        let imp = self.imp();
        let mediainfobox = imp.mediainfobox.get();
        let mediainforevealer = imp.mediainforevealer.get();

        while let Some(child) = mediainfobox.last_child() {
            mediainfobox.remove(&child)
        }

        for mediasource in media_sources {
            let singlebox = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(16)
                .build();
            singlebox.add_css_class("media-info-source");

            let mediascrolled = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Automatic)
                .vscrollbar_policy(gtk::PolicyType::Never)
                .overlay_scrolling(true)
                .build();

            let mediascrolled = mediascrolled.fix();
            mediascrolled.add_css_class("media-info-streams-scroll");

            let mediabox = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .halign(gtk::Align::Start)
                .spacing(18)
                .build();
            mediabox.add_css_class("media-info-streams");
            for mediapart in &mediasource.media_streams {
                if mediapart.stream_type == "Attachment" {
                    continue;
                }
                mediabox.append(&Self::media_stream_card(mediapart));
            }

            mediascrolled.set_child(Some(&mediabox));
            singlebox.append(mediascrolled);
            singlebox.append(&Self::media_source_summary(&mediasource, date_created));
            mediainfobox.append(&singlebox);
        }
        mediainforevealer.set_reveal_child(true);
    }

    fn media_stream_card(stream: &MediaStream) -> gtk::Box {
        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .width_request(360)
            .build();
        card.add_css_class("media-info-card");

        let header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();
        header.add_css_class("media-info-card-header");

        let icon = gtk::Image::builder()
            .icon_name(Self::media_stream_icon(&stream.stream_type))
            .valign(gtk::Align::Center)
            .build();
        header.append(&icon);

        let stream_type = gettext(stream.stream_type.as_str());
        let title = gtk::Label::builder()
            .label(&stream_type)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        title.add_css_class("media-info-card-title");
        header.append(&title);
        card.append(&header);

        if let Some(display_title) = stream.display_title.as_ref().filter(|s| !s.is_empty()) {
            let subtitle = gtk::Label::builder()
                .label(display_title)
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .build();
            subtitle.add_css_class("media-info-card-subtitle");
            card.append(&subtitle);
        }

        let grid = gtk::Grid::builder()
            .column_spacing(16)
            .row_spacing(8)
            .hexpand(true)
            .build();
        grid.add_css_class("media-info-grid");
        let mut row = 0;

        Self::append_media_info_row(&grid, &mut row, &gettext("Title"), stream.title.as_deref());
        Self::append_media_info_row(&grid, &mut row, &gettext("Codec"), stream.codec.as_deref());
        Self::append_media_info_row(
            &grid,
            &mut row,
            &gettext("Language"),
            stream
                .display_language
                .as_deref()
                .or(stream.language.as_deref()),
        );
        if let (Some(width), Some(height)) = (stream.width, stream.height) {
            let resolution = format!("{width}x{height}");
            Self::append_media_info_row(
                &grid,
                &mut row,
                &gettext("Resolution"),
                Some(resolution.as_str()),
            );
        }
        if let Some(frame_rate) = stream.average_frame_rate {
            let frame_rate = format!("{frame_rate:.2}");
            Self::append_media_info_row(
                &grid,
                &mut row,
                &gettext("AverageFrameRate"),
                Some(frame_rate.as_str()),
            );
        }
        if let Some(bit_rate) = stream.bit_rate {
            let bit_rate = Self::format_bit_rate(bit_rate);
            Self::append_media_info_row(
                &grid,
                &mut row,
                &gettext("Bitrate"),
                Some(bit_rate.as_str()),
            );
        }
        if let Some(channel_layout) = stream.channel_layout.as_deref() {
            Self::append_media_info_row(
                &grid,
                &mut row,
                &gettext("ChannelLayout"),
                Some(channel_layout),
            );
        }
        if let Some(channels) = stream.channels {
            let channels = channels.to_string();
            Self::append_media_info_row(
                &grid,
                &mut row,
                &gettext("Channel"),
                Some(channels.as_str()),
            );
        }
        if let Some(sample_rate) = stream.sample_rate {
            let sample_rate = format!("{sample_rate}Hz");
            Self::append_media_info_row(
                &grid,
                &mut row,
                &gettext("SampleRate"),
                Some(sample_rate.as_str()),
            );
        }
        if let Some(bit_depth) = stream.bit_depth {
            let bit_depth = bit_depth.to_string();
            Self::append_media_info_row(
                &grid,
                &mut row,
                &gettext("BitDepth"),
                Some(bit_depth.as_str()),
            );
        }
        Self::append_media_info_row(
            &grid,
            &mut row,
            &gettext("ColorSpace"),
            stream.color_space.as_deref(),
        );
        Self::append_media_info_row(
            &grid,
            &mut row,
            &gettext("PixelFormat"),
            stream.pixel_format.as_deref(),
        );
        let external = Self::format_yes_no(stream.is_external);
        Self::append_media_info_row(
            &grid,
            &mut row,
            &gettext("External"),
            Some(external.as_str()),
        );

        card.append(&grid);
        card
    }

    fn append_media_info_row(grid: &gtk::Grid, row: &mut i32, key: &str, value: Option<&str>) {
        let Some(value) = value.filter(|value| !value.is_empty()) else {
            return;
        };

        let key_label = gtk::Label::builder()
            .label(key)
            .xalign(0.0)
            .valign(gtk::Align::Start)
            .build();
        key_label.add_css_class("media-info-key");

        let value_label = gtk::Label::builder()
            .label(value)
            .xalign(0.0)
            .hexpand(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .valign(gtk::Align::Start)
            .build();
        value_label.add_css_class("media-info-value");

        grid.attach(&key_label, 0, *row, 1, 1);
        grid.attach(&value_label, 1, *row, 1, 1);
        *row += 1;
    }

    fn media_source_summary(source: &MediaSource, date_created: Option<DateTime<Utc>>) -> gtk::Box {
        let summary = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .halign(gtk::Align::Fill)
            .build();
        summary.add_css_class("media-info-summary");

        let name = if source.name.is_empty() {
            source
                .path
                .as_deref()
                .and_then(|path| path.rsplit(['/', '\\']).next())
                .unwrap_or_default()
                .to_string()
        } else {
            source.name.clone()
        };
        let title = gtk::Label::builder()
            .label(&name)
            .xalign(0.5)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .build();
        title.add_css_class("media-info-summary-title");
        summary.append(&title);

        let mut details = Vec::new();
        if let Some(container) = source.container.as_ref().filter(|s| !s.is_empty()) {
            details.push(container.to_uppercase());
        }
        if let Some(size) = source.size {
            details.push(bytefmt::format(size));
        }
        if let Some(bit_rate) = source.bit_rate {
            details.push(Self::format_bit_rate(bit_rate));
        }
        let created = dt(date_created);
        if !created.is_empty() {
            details.push(created);
        }
        let subtitle = gtk::Label::builder()
            .label(details.join("  "))
            .xalign(0.5)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        subtitle.add_css_class("media-info-summary-subtitle");
        summary.append(&subtitle);

        summary
    }

    fn media_stream_icon(stream_type: &str) -> &'static str {
        match stream_type {
            "Video" => "video-x-generic-symbolic",
            "Audio" => "audio-x-generic-symbolic",
            "Subtitle" => "media-view-subtitles-symbolic",
            _ => "text-x-generic-symbolic",
        }
    }

    fn format_bit_rate(bit_rate: u64) -> String {
        if bit_rate >= 1_000_000 {
            format!("{:.1}Mbps", bit_rate as f64 / 1_000_000.0)
        } else if bit_rate >= 1_000 {
            format!("{:.0}Kbps", bit_rate as f64 / 1_000.0)
        } else {
            format!("{bit_rate}bps")
        }
    }

    fn format_yes_no(value: bool) -> String {
        if value { gettext("Yes") } else { gettext("No") }
    }

    pub async fn setactorscrolled(&self, actors: Vec<SimpleListItem>) {
        let hortu = self.imp().actorhortu.get();
        hortu.set_items(actors);
    }

    pub async fn set_lists(&self, id: &str) {
        self.sets("Recommend", id).await;
        self.sets("Included In", id).await;
    }

    pub async fn sets(&self, types: &str, id: &str) {
        let hortu = match types {
            "Recommend" => self.imp().recommendhortu.get(),
            "Included In" => self.imp().includehortu.get(),
            "Additional Parts" => self.imp().additionalhortu.get(),
            _ => return,
        };

        let id = id.to_string();
        let types = types.to_string();

        let mut events = fetch_with_cache(
            &format!("item_{types}_{id}"),
            CachePolicy::ReadCacheAndRefresh,
            async move {
                match types.as_str() {
                    "Recommend" => JELLYFIN_CLIENT.get_similar(&id).await,
                    "Included In" => JELLYFIN_CLIENT.get_included(&id).await,
                    "Additional Parts" => JELLYFIN_CLIENT.get_additional(&id).await,
                    _ => Ok(List::default()),
                }
            },
        )
        .await;

        hortu.set_unify_size(UnifySize::Majority);

        while let Some(event) = events.recv().await {
            match event {
                CacheEvent::Data { data, .. } => {
                    hortu.set_items(data.items);
                }
                CacheEvent::Error(e) => {
                    self.toast(e.to_user_facing());
                }
            }
        }
    }

    pub fn set_flowbuttons(&self, infos: Vec<SGTitem>, type_: &str) {
        let imp = self.imp();
        let horbu = match type_ {
            "Genres" => imp.genreshorbu.get(),
            "Studios" => imp.studioshorbu.get(),
            "Tags" => imp.tagshorbu.get(),
            _ => return,
        };

        horbu.set_items(&infos, type_);
    }

    pub fn set_flowlinks(&self, links: Vec<Urls>) {
        self.imp().linkshorbu.set_links(&links);
    }

    pub fn window(&self) -> Window {
        self.root().unwrap().downcast::<Window>().unwrap()
    }

    #[template_callback]
    async fn play_cb(&self) {
        let video_dropdown = self.imp().namedropdown.get();
        let sub_dropdown = self.imp().subdropdown.get();

        let Some(video_object) = video_dropdown
            .selected_item()
            .and_downcast::<glib::BoxedAnyObject>()
        else {
            self.toast(gettext("No video source found"));
            return;
        };

        let sub_dl = sub_dropdown
            .selected_item()
            .and_downcast::<glib::BoxedAnyObject>()
            .map(|obj| obj.borrow::<DropdownList>().to_owned());

        let video_dl: std::cell::Ref<DropdownList> = video_object.borrow();
        let (sub_index, sub_lang) = sub_dl
            .map(|sub_dl| {
                (
                    sub_dl.index,
                    sub_dl.sub_lang.to_owned(),
                )
            })
            .unwrap_or_default();

        let info = SelectedVideoSubInfo {
            sub_index,
            video_index: video_dl.index.unwrap_or_default(),
            sub_lang,
            media_source_id: video_dl.id.to_owned().unwrap_or_default(),
        };

        let item = self.current_item().unwrap_or(self.item());
        let start_seconds = item.playback_position_ticks() as f64 / 10_000_000.0;

        let episode_list = self.imp().episode_list_vec.borrow();
        let episode_list: Vec<TuItem> = episode_list
            .iter()
            .map(|item| TuItem::from_simple(item.to_owned()))
            .collect();

        let matcher = self.imp().video_version_matcher.borrow().to_owned();

        self.window()
            .play_media(Some(info), item, episode_list, matcher, start_seconds);
    }

    fn set_control_opacity(&self, opacity: f64) {
        let imp = self.imp();
        imp.left_button.set_opacity(opacity);
        imp.right_button.set_opacity(opacity);
    }

    fn are_controls_visible(&self) -> bool {
        if self.hide_controls_animation().state() == adw::AnimationState::Playing {
            return false;
        }

        self.imp().left_button.opacity() >= 0.68
            || self.show_controls_animation().state() == adw::AnimationState::Playing
    }

    fn show_controls_animation(&self) -> &adw::TimedAnimation {
        self.imp().show_button_animation.get_or_init(|| {
            let target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |opacity| obj.set_control_opacity(opacity)
            ));

            adw::TimedAnimation::builder()
                .duration(SHOW_BUTTON_ANIMATION_DURATION)
                .widget(&self.imp().scrolled.get())
                .target(&target)
                .value_to(1.)
                .build()
        })
    }

    fn hide_controls_animation(&self) -> &adw::TimedAnimation {
        self.imp().hide_button_animation.get_or_init(|| {
            let target = adw::CallbackAnimationTarget::new(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |opacity| obj.set_control_opacity(opacity)
            ));

            adw::TimedAnimation::builder()
                .duration(SHOW_BUTTON_ANIMATION_DURATION)
                .widget(&self.imp().scrolled.get())
                .target(&target)
                .value_to(0.)
                .build()
        })
    }

    #[template_callback]
    fn on_rightbutton_clicked(&self) {
        self.anime::<true>();
    }

    fn controls_opacity(&self) -> f64 {
        self.imp().left_button.opacity()
    }

    #[template_callback]
    fn on_enter_focus(&self) {
        if !self.are_controls_visible() {
            self.hide_controls_animation().pause();
            self.show_controls_animation()
                .set_value_from(self.controls_opacity());
            self.show_controls_animation().play();
        }
    }

    #[template_callback]
    fn on_leave_focus(&self) {
        if self.are_controls_visible() {
            self.show_controls_animation().pause();
            self.hide_controls_animation()
                .set_value_from(self.controls_opacity());
            self.hide_controls_animation().play();
        }
    }

    #[template_callback]
    fn on_leftbutton_clicked(&self) {
        self.anime::<false>();
    }

    #[template_callback]
    async fn on_season_view_more_clicked(&self) {
        let object = self.imp().seasonlist.selected_item();
        let Some(season_name) = object.and_downcast_ref::<gtk::StringObject>() else {
            return;
        };

        let season_name = season_name.string().to_string();

        let season_list = self.imp().season_list_vec.borrow();
        let Some(season) = season_list.iter().find(|s| s.name == season_name) else {
            self.toast(gettext(
                "Season not found. Is this a continue watching list?",
            ));
            return;
        };

        let item = TuItem::from_simple(season.to_owned());
        item.activate(self);
    }

    fn anime<const R: bool>(&self) {
        let scrolled = self.imp().scrolled.get();
        let adj = scrolled.hadjustment();

        let Some(clock) = scrolled.frame_clock() else {
            return;
        };

        let start = adj.value();
        let end = if R { start + 800.0 } else { start - 800.0 };

        let start_time = clock.frame_time();
        let end_time = start_time + 1000 * 400;

        scrolled.add_tick_callback(move |_view, clock| {
            let now = clock.frame_time();
            if now < end_time && adj.value() != end {
                let mut t = (now - start_time) as f64 / (end_time - start_time) as f64;
                t = Self::ease_in_out_cubic(t);
                adj.set_value(start + t * (end - start));
                glib::ControlFlow::Continue
            } else {
                adj.set_value(end);
                glib::ControlFlow::Break
            }
        });
    }

    fn ease_in_out_cubic(t: f64) -> f64 {
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            let t = 2.0 * t - 2.0;
            0.5 * t * t * t + 1.0
        }
    }
}

pub fn dt(date: Option<chrono::DateTime<Utc>>) -> String {
    let Some(date) = date else {
        return "".to_string();
    };
    date.naive_local().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[derive(Debug, Clone)]
pub struct SelectedVideoSubInfo {
    pub sub_lang: Option<String>,
    pub sub_index: Option<i64>,
    pub video_index: i64,
    pub media_source_id: String,
}
