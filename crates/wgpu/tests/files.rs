//! Cache files are named by their pipeline set, never shrink, replace a refused file, and a
//! directory keeps the four of an adapter used most recently.

mod common;

use std::time::Duration;

#[test]
fn each_pipeline_set_keeps_a_file_of_its_own() {
    let Some(device) = common::pipeline_cache::device() else {
        return;
    };
    let directory = common::pipeline_cache::directory();
    let first = common::pipeline_cache::load(&device, &directory, 1);
    common::pipeline_cache::build(&device, &first, 0, 2);
    first.store();
    let kept = common::pipeline_cache::files(&directory);

    let second = common::pipeline_cache::load(&device, &directory, 2);
    common::pipeline_cache::build(&device, &second, 10, 1);
    second.store();

    let files = common::pipeline_cache::files(&directory);
    assert_eq!(
        files.len(),
        2,
        "two pipeline sets keep two files: {files:?}"
    );
    assert!(
        files.contains(&kept[0]),
        "the first set's file is left as it was: {files:?}"
    );
}

#[test]
fn a_cache_that_holds_less_never_replaces_the_file() {
    let Some(device) = common::pipeline_cache::device() else {
        return;
    };
    let Some(other) = device.beside() else {
        return;
    };
    let directory = common::pipeline_cache::directory();
    // Two processes load the same missing file at once.
    let larger = common::pipeline_cache::load(&device, &directory, 1);
    let smaller = common::pipeline_cache::load(&other, &directory, 1);
    common::pipeline_cache::build(&device, &larger, 100, 4);
    common::pipeline_cache::build(&other, &smaller, 200, 1);
    larger.store();
    let written = common::pipeline_cache::files(&directory);

    smaller.store();

    assert_eq!(
        common::pipeline_cache::files(&directory),
        written,
        "the smaller cache leaves the larger file"
    );
}

#[test]
fn a_refused_file_is_replaced() {
    let Some(device) = common::pipeline_cache::device() else {
        return;
    };
    let directory = common::pipeline_cache::directory();
    let cache = common::pipeline_cache::load(&device, &directory, 1);
    common::pipeline_cache::build(&device, &cache, 300, 1);
    cache.store();
    let (name, _) = common::pipeline_cache::files(&directory).remove(0);
    let garbage = vec![0x5a; 1 << 20];
    std::fs::write(directory.join(&name), &garbage).expect("the file is overwritten");

    let cache = common::pipeline_cache::load(&device, &directory, 1);
    common::pipeline_cache::build(&device, &cache, 301, 1);
    cache.store();

    let stored = std::fs::read(directory.join(&name)).expect("the file reads");
    assert_ne!(stored, garbage, "the refused file is replaced");
}

#[test]
fn a_directory_keeps_the_four_files_used_most_recently() {
    let Some(device) = common::pipeline_cache::device() else {
        return;
    };
    let directory = common::pipeline_cache::directory();
    let mut names = Vec::new();
    for pipelines in 1..=4 {
        let cache = common::pipeline_cache::load(&device, &directory, pipelines);
        common::pipeline_cache::build(&device, &cache, 400 + pipelines as u32, 1);
        cache.store();
        let new = common::pipeline_cache::files(&directory)
            .into_iter()
            .map(|(name, _)| name)
            .find(|name| !names.contains(name))
            .expect("each set writes a file");
        names.push(new);
        std::thread::sleep(Duration::from_millis(20));
    }
    // Loading the first set's file counts as using it, though nothing is written.
    drop(common::pipeline_cache::load(&device, &directory, 1));
    std::thread::sleep(Duration::from_millis(20));

    let cache = common::pipeline_cache::load(&device, &directory, 5);
    common::pipeline_cache::build(&device, &cache, 405, 1);
    cache.store();

    let kept: Vec<String> = common::pipeline_cache::files(&directory)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(kept.len(), 4, "the directory keeps four files: {kept:?}");
    assert!(
        kept.contains(&names[0]),
        "the file loaded last is kept: {kept:?}"
    );
    assert!(
        !kept.contains(&names[1]),
        "the file used least recently is removed: {kept:?}"
    );
}
