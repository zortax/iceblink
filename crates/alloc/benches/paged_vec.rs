//! Read cost of a sparse side table for absent, present and out-of-range slots.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use iceblink_alloc::arena::{Generation, Key, NamespaceId, PAGE_LEN, PagedVec};

/// The value type the keys refer to.
struct Value;

fn key(index: u32) -> Key<Value> {
    Key::new(index, Generation::FIRST, NamespaceId::FIRST)
}

/// A table with one entry on every second page.
fn scattered(pages: u32) -> PagedVec<Key<Value>, u32> {
    let mut table = PagedVec::for_namespace(NamespaceId::FIRST);
    for page in 0..pages {
        *table.get_mut(key(page * PAGE_LEN as u32 * 2)) = page;
    }
    table
}

fn benches(c: &mut Criterion) {
    let table = scattered(64);

    c.bench_function("paged_vec/absent", |b| {
        // Every slot read here lies on an unallocated page.
        let mut index = PAGE_LEN as u32;
        b.iter(|| {
            index = index.wrapping_add(PAGE_LEN as u32 * 2);
            black_box(table.get(key(black_box(index))))
        });
    });

    c.bench_function("paged_vec/present", |b| {
        let mut index = 0_u32;
        b.iter(|| {
            index = (index + PAGE_LEN as u32 * 2) % (PAGE_LEN as u32 * 128);
            black_box(table.get(key(black_box(index))))
        });
    });

    c.bench_function("paged_vec/beyond_the_index", |b| {
        b.iter(|| black_box(table.get(key(black_box(u32::MAX)))));
    });
}

criterion_group!(paged_vec, benches);
criterion_main!(paged_vec);
