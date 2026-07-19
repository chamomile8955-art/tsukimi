use adw::prelude::BinExt;
use gtk::prelude::*;

use crate::ui::{
    provider::tu_item::{PreferPoster, TuItem},
    widgets::{
        hover_scale::HoverScale, picture_loader::PictureLoader, tu_list_item::imp::PosterType,
    },
};

use super::TuItemBasic;

pub trait TuItemOverlayPrelude {
    fn get_image_request(&self, item: &TuItem) -> (&str, Option<String>, Option<String>, String) {
        if self.poster_type_ext() != PosterType::Poster
            && let Some(imag_tags) = item.image_tags()
        {
            match self.poster_type_ext() {
                PosterType::Banner => {
                    if let Some(image_tag) = imag_tags.banner() {
                        return ("Banner", None, Some(image_tag), item.id());
                    } else if let Some(image_tag) = imag_tags.thumb() {
                        return ("Thumb", None, Some(image_tag), item.id());
                    } else if let Some(image_tag) = imag_tags.backdrop() {
                        return ("Backdrop", Some(0.to_string()), Some(image_tag), item.id());
                    }
                }
                PosterType::Backdrop => {
                    if let Some(image_tag) = imag_tags.backdrop() {
                        return ("Backdrop", Some(0.to_string()), Some(image_tag), item.id());
                    } else if let Some(image_tag) = imag_tags.thumb() {
                        return ("Thumb", None, Some(image_tag), item.id());
                    }
                }
                _ => {}
            }
        }
        match item.prefer_poster() {
            // Continue Watching, use parent video poster if possible
            PreferPoster::ParentVideo => {
                if let Some(parent_thumb_item_id) = item.parent_thumb_item_id() {
                    ("Thumb", None, None, parent_thumb_item_id)
                } else if let Some(parent_backdrop_item_id) = item.parent_backdrop_item_id() {
                    (
                        "Backdrop",
                        Some(0.to_string()),
                        None,
                        parent_backdrop_item_id,
                    )
                } else {
                    (
                        "Backdrop",
                        Some(0.to_string()),
                        item.image_tags().and_then(|tags| tags.backdrop()),
                        item.id(),
                    )
                }
            }
            // Latest, use parent primary image if possible, this is for latest episodes
            PreferPoster::ParentPost
                if let Some(parent_backdrop_item_id) = item.parent_backdrop_item_id() =>
            {
                ("Primary", None, None, parent_backdrop_item_id)
            }
            _ => {
                if let Some(img_tags) = item.primary_image_item_id() {
                    // use primary image if possible
                    (
                        "Primary",
                        None,
                        item.image_tags().and_then(|tags| tags.primary()),
                        img_tags,
                    )
                } else if item.image_tags().is_none_or(|t| t.all_none())
                    && let Some(parent_backdrop_item_id) = item.parent_backdrop_item_id()
                {
                    // fallback to parent backdrop if no image tags and parent backdrop exists, this
                    // is for some season items that don't have image tags
                    ("Primary", None, None, parent_backdrop_item_id)
                } else {
                    // finally fallback to primary image with item id
                    (
                        "Primary",
                        None,
                        item.image_tags().and_then(|tags| tags.primary()),
                        item.id(),
                    )
                }
            }
        }
    }

    fn overlay(&self) -> gtk::Overlay;

    fn poster_type_ext(&self) -> PosterType;
}

pub trait TuItemOverlay: TuItemBasic + TuItemOverlayPrelude {
    fn set_picture(&self);

    fn set_picture_with_hover_scale(&self);

    fn set_animated_picture(&self);
}

impl<T> TuItemOverlay for T
where
    T: TuItemBasic + TuItemOverlayPrelude,
{
    fn set_picture(&self) {
        let item = self.item();
        let (image_type, index, image_tag, id) = self.get_image_request(&item);
        let overlay = self.overlay();

        if let Some(picture_loader) = overlay.child().and_downcast::<PictureLoader>() {
            picture_loader.set_fallback_title(item.name());
            picture_loader.reload_with_image_tag(&id, image_type, index, image_tag, false);
            return;
        }

        let picture_loader = PictureLoader::new_with_image_tag(&id, image_type, index, image_tag);
        picture_loader.set_fallback_title(item.name());
        picture_loader.add_css_class("inbox");
        overlay.set_child(Some(&picture_loader));
    }

    fn set_picture_with_hover_scale(&self) {
        let item = self.item();
        let (image_type, index, image_tag, id) = self.get_image_request(&item);
        let overlay = self.overlay();

        if let Some(hover_scale) = overlay.child().and_downcast::<HoverScale>()
            && let Some(picture_loader) = hover_scale.child().and_downcast::<PictureLoader>()
        {
            picture_loader.set_fallback_title(item.name());
            picture_loader.reload_with_image_tag(&id, image_type, index, image_tag, false);
            return;
        }

        let picture_loader = PictureLoader::new_with_image_tag(&id, image_type, index, image_tag);
        picture_loader.set_fallback_title(item.name());
        let hover_scale = HoverScale::new();
        hover_scale.set_child(Some(&picture_loader));
        overlay.set_child(Some(&hover_scale));
    }

    fn set_animated_picture(&self) {
        let item = self.item();
        let (image_type, index, image_tag, id) = self.get_image_request(&item);
        let overlay = self.overlay();

        if let Some(picture_loader) = overlay.child().and_downcast::<PictureLoader>() {
            picture_loader.set_fallback_title(item.name());
            picture_loader.reload_with_image_tag(&id, image_type, index, image_tag, true);
            return;
        }

        let picture_loader =
            PictureLoader::new_animated_with_image_tag(&id, image_type, index, image_tag);
        picture_loader.set_fallback_title(item.name());
        picture_loader.add_css_class("inbox");
        overlay.set_child(Some(&picture_loader));
    }
}
