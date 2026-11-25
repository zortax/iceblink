//! Recording transactions and taking them apart.

mod common;

use std::sync::Arc;

use iceblink::{
    AnyId, ArenaRange, ChunkKey, ClipNode, ClipRef, ClipShape, Color, Command, Filter, FontId,
    Glyph, GlyphFlags, GlyphRun, ItemKind, LocalClip, LocalParent, MarkPayload, OrderEntry,
    OrderNode, OutputId, Paint, PaintKind, PaintRef, ROUND, SpaceRef, Transaction,
};

fn run(glyphs: ArenaRange, coords: ArenaRange) -> GlyphRun {
    GlyphRun {
        font: FontId(3),
        size: 12.0,
        glyphs,
        coords,
        paint: PaintRef::NONE,
        embolden: 0.0,
        skew: 0.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: GlyphFlags::HINT,
        _pad: [0],
        palette: 0,
    }
}

#[test]
fn a_recording_keeps_record_order_across_kinds() {
    let mut transaction = Transaction::new(common::ids());
    let props = common::props(&mut transaction);
    let mut recorder = transaction.record(ChunkKey::new(9, 1), props);

    let first = recorder.quad(common::quad(0.0));
    let glyphs = recorder.glyphs(
        &[Glyph {
            id: 1,
            x: 0.0,
            y: 10.0,
        }; 3],
    );
    let coords = recorder.coords(&[100, -200]);
    let second = recorder.glyph_run(run(glyphs, coords));
    let third = recorder.quad(common::quad(20.0));
    let more = recorder.glyphs(
        &[Glyph {
            id: 2,
            x: 5.0,
            y: 10.0,
        }; 2],
    );

    let parts = transaction.into_parts();
    let chunk = &parts.chunks[0];

    assert_eq!((first, second, third), (0, 1, 2));
    assert_eq!(
        (glyphs, coords),
        (
            ArenaRange { start: 0, len: 3 },
            ArenaRange { start: 0, len: 2 }
        )
    );
    assert_eq!(more, ArenaRange { start: 3, len: 2 });
    assert_eq!(chunk.key, ChunkKey::new(9, 1));
    assert_eq!(
        chunk
            .ops
            .iter()
            .map(|op| (op.kind, op.index))
            .collect::<Vec<_>>(),
        [
            (ItemKind::QUAD, 0),
            (ItemKind::GLYPH_RUN, 0),
            (ItemKind::QUAD, 1)
        ]
    );
    assert_eq!(chunk.quads[1].bounds[0], 20.0);
    assert_eq!(chunk.glyph_runs[0].coords, coords);
    assert_eq!(chunk.glyphs.len(), 5);
    assert_eq!(chunk.coords, [100, -200]);
}

#[test]
fn lists_return_their_indices_and_local_clips_set_the_local_bit() {
    let mut transaction = Transaction::new(common::ids());
    let props = common::props(&mut transaction);
    let mut recorder = transaction.record(ChunkKey::new(1, 0), props);
    let clip = |parent| LocalClip {
        parent,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 5.0, 5.0],
            radii: [1.0; 8],
            shapes: ROUND,
        },
    };

    let outer = recorder.local_clip(clip(LocalParent::Chunk));
    let inner = recorder.local_clip(clip(LocalParent::Local(0)));
    let payload = recorder.payload(Arc::new(MarkPayload::default()));
    let filters = recorder.filters(Arc::from([Filter::Blur(1.0)]));
    let chain = recorder.filters(Arc::from([Filter::Opacity(0.5)]));

    assert_eq!(outer, ClipRef(ClipRef::LOCAL));
    assert_eq!(inner.local_index(), Some(1));
    assert_eq!(ClipRef::local(1 << 24 | 3).local_index(), Some(3));
    assert_eq!(ClipRef::INHERIT.local_index(), None);
    assert_eq!(ClipRef::NONE.local_index(), None);
    assert_eq!((payload, filters, chain), (0, 0, 1));
}

#[test]
fn hints_widen_to_the_bounding_box_of_every_call() {
    let mut transaction = Transaction::new(common::ids());
    let props = common::props(&mut transaction);
    let mut recorder = transaction.record(ChunkKey::new(1, 0), props);

    recorder.hint([0.0, 0.0, 10.0, 10.0]);
    recorder.hint([20.0, 5.0, 10.0, 10.0]);

    assert_eq!(
        transaction.into_parts().chunks[0].hint,
        Some([0.0, 0.0, 30.0, 15.0])
    );
}

#[test]
fn commands_keep_issue_order_and_minted_ids_keep_mint_order() {
    let mut transaction = Transaction::new(common::ids());
    assert!(transaction.is_empty());

    let space = transaction.create_transform(common::root_node());
    let root = transaction.create_order(OrderNode::default());
    let child = transaction.create_order(OrderNode::default());
    let key = ChunkKey::new(4, 2);
    transaction.splice(
        root,
        0,
        0,
        &[OrderEntry::Chunk(key), OrderEntry::Node(child)],
    );
    transaction.splice(child, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(5, 0))]);
    transaction.set_root(OutputId(0), Some(root));
    transaction.set_offset(key, [2.0, 3.0]);
    transaction.remove(space);

    assert_eq!(transaction.client(), common::ids().client());
    let parts = transaction.into_parts();

    assert_eq!(
        parts.minted,
        [
            AnyId::Transform(space),
            AnyId::Order(root),
            AnyId::Order(child)
        ]
    );
    assert_eq!(parts.entries.len(), 3);
    let kinds: Vec<_> = parts
        .commands
        .iter()
        .map(|command| match command {
            Command::CreateTransform(..) => "create_transform",
            Command::CreateOrder(..) => "create_order",
            Command::Splice { insert, .. } if insert.start == 0 && insert.len == 2 => "splice 0..2",
            Command::Splice { insert, .. } if insert.start == 2 && insert.len == 1 => "splice 2..3",
            Command::SetRoot(OutputId(0), Some(id)) if *id == root => "set_root",
            Command::SetOffset(k, [2.0, 3.0]) if *k == key => "set_offset",
            Command::Remove(AnyId::Transform(id)) if *id == space => "remove",
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "create_transform",
            "create_order",
            "create_order",
            "splice 0..2",
            "splice 2..3",
            "set_root",
            "set_offset",
            "remove",
        ]
    );
}

#[test]
fn a_key_recorded_twice_keeps_both_recordings_in_issue_order() {
    let mut transaction = Transaction::new(common::ids());
    let props = common::props(&mut transaction);
    let key = ChunkKey::new(1, 0);

    transaction.record(key, props).quad(common::quad(0.0));
    transaction.record(key, props).quad(common::quad(50.0));
    let parts = transaction.into_parts();

    let records: Vec<_> = parts
        .commands
        .iter()
        .filter_map(|command| match command {
            Command::Record(index) => Some(*index),
            _ => None,
        })
        .collect();
    assert_eq!(records, [0, 1]);
    assert_eq!(parts.chunks[1].quads[0].bounds[0], 50.0);
}

#[test]
fn transactions_and_their_parts_are_send() {
    fn assert_send<T: Send>() {}

    assert_send::<Transaction>();
    assert_send::<iceblink::TransactionParts>();
}

#[test]
fn references_return_slots_and_list_each_id_once_per_run_of_calls() {
    let mut transaction = Transaction::new(common::ids());
    let props = common::props(&mut transaction);
    let paint = transaction.create_paint(Paint::Solid(Color::TRANSPARENT));
    let clip = transaction.create_clip(ClipNode {
        parent: None,
        space: props.space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 5.0, 5.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let mut recorder = transaction.record(ChunkKey::new(1, 0), props);

    let fill = recorder.paint_ref(PaintKind::SOLID, paint);
    let again = recorder.paint_ref(PaintKind::SOLID, paint);
    let space = recorder.space_ref(props.space);
    let clipped = recorder.clip_ref(clip);

    assert_eq!(fill, again);
    assert_eq!((fill.kind, fill.index), (PaintKind::SOLID, paint.slot()));
    assert_eq!(space, SpaceRef(props.space.slot()));
    assert_eq!(clipped, ClipRef(clip.slot()));
    assert_eq!(
        transaction.into_parts().chunks[0].refs,
        [
            AnyId::Paint(paint),
            AnyId::Transform(props.space),
            AnyId::Clip(clip),
        ]
    );
}

#[test]
fn chunk_keys_compare_by_owner_and_part_alone() {
    let padded = ChunkKey {
        _pad: 9,
        ..ChunkKey::new(1, 2)
    };

    assert_eq!(padded, ChunkKey::new(1, 2));
    assert!(ChunkKey::new(1, 3) < ChunkKey::new(2, 0));
}

#[test]
fn a_chunk_filled_apart_from_its_transaction_records_like_one_recorded_in_place() {
    let mut transaction = Transaction::new(common::ids());
    let props = common::props(&mut transaction);
    let key = ChunkKey::new(3, 0);

    let mut chunk = transaction.empty_chunk(key, props);
    let mut recorder = chunk.recorder();
    recorder.quad(common::quad(0.0));
    let space = transaction.create_transform(common::root_node());
    let at = recorder.space_ref(space);
    let mut quad = common::quad(10.0);
    quad.space = at;
    recorder.quad(quad);
    transaction.record_chunk(chunk);

    let parts = transaction.into_parts();

    assert!(matches!(parts.commands.last(), Some(Command::Record(0))));
    assert_eq!(parts.chunks[0].key, key);
    assert_eq!(parts.chunks[0].ops.len(), 2);
    assert_eq!(parts.chunks[0].refs, [AnyId::from(space)]);
}
