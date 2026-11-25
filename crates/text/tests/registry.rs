//! Font ids shared between layout and the compositor.

mod common;

use std::sync::Arc;

use iceblink::FontData;
use iceblink::FontRegistry as _;
use iceblink_testing::fonts::{LATIN, read_font};
use iceblink_text::typeset::{Alignment, FontRegistry, Property, TextItem, Typesetter};

use crate::common::typeset::SANS;

#[test]
fn equal_bytes_register_once_and_the_file_is_shared() {
    let registry = FontRegistry::without_system_fonts();
    let data = read_font(LATIN);
    let id = registry.register(data.clone());
    let copy = FontData::new(data.bytes().to_vec(), 0);
    assert_eq!(registry.register(copy), id);
    assert_eq!(registry.generation(), 1);

    let shared = registry.get(id).expect("the font is registered");
    assert!(Arc::ptr_eq(&shared.file, &data.file));
}

#[test]
fn a_typesetter_finds_fonts_registered_after_it_by_their_registered_id() {
    let registry = Arc::new(FontRegistry::without_system_fonts());
    let mut typesetter = Typesetter::new(Arc::clone(&registry));
    let earlier = registry.register(read_font(iceblink_testing::fonts::VARIABLE));
    let latin = registry.register(read_font(LATIN));
    assert_ne!(earlier, latin);

    let mut text = typesetter.text("Same id");
    text.style(Property::Family(SANS));
    let layout = text.build(None, Alignment::Start);
    let fonts: Vec<_> = layout
        .items()
        .filter_map(|item| match item {
            TextItem::Run(run) => Some(run.font),
            TextItem::Line(_) => None,
        })
        .collect();
    assert_eq!(fonts, vec![latin]);
    assert_eq!(registry.generation(), 2);
}
