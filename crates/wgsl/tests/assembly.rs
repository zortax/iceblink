//! Assembling modules and application shaders from parts.

use iceblink::ShaderMode;
use iceblink::shader::entry;
use iceblink_wgsl::{MODULES, PART_NAMES, effect, epilogue, fragment_entry, module, part};

const MODES: [ShaderMode; 3] = [ShaderMode::PAINT, ShaderMode::COVERAGE, ShaderMode::FILTER];

#[test]
fn every_named_part_has_text() {
    for name in PART_NAMES {
        assert!(!part(name).trim().is_empty(), "{name} is empty");
    }
}

#[test]
#[should_panic(expected = "no part is called")]
fn an_unknown_part_panics() {
    part("not-a-part");
}

#[test]
fn every_module_names_parts_that_exist() {
    for (name, parts) in MODULES {
        for piece in *parts {
            assert!(!part(piece).is_empty(), "{name} names `{piece}`");
        }
        assert!(module(name).is_some(), "{name} does not assemble");
    }
}

#[test]
fn an_unknown_module_assembles_to_nothing() {
    assert!(module("not-a-module").is_none());
}

#[test]
fn a_shader_holds_its_snippet_between_the_prelude_and_the_epilogue() {
    let source = effect(ShaderMode::PAINT, "// the application's text").unwrap();
    let snippet = source.find("// the application's text").unwrap();
    let prelude = source.find("fn superellipse_sdf(").unwrap();
    let epilogue = source.find("fn cr_fs_shader_paint(").unwrap();
    assert!(prelude < snippet);
    assert!(snippet < epilogue);
}

#[test]
fn a_filter_is_assembled_from_the_prelude_alone() {
    let source = effect(ShaderMode::FILTER, "// the application's text").unwrap();
    assert!(source.contains("struct ShaderInput"));
    assert!(source.contains("fn cr_fs_shader_filter("));
    assert!(!source.contains("struct cr_Bounds"));
}

#[test]
fn an_unknown_mode_assembles_to_nothing() {
    assert!(effect(ShaderMode(9), "").is_none());
    assert!(epilogue(ShaderMode(9)).is_none());
}

#[test]
fn every_mode_has_one_fragment_stage_and_its_own_entry_points() {
    let mut entries: Vec<&str> = MODES.iter().map(|mode| entry(*mode).unwrap()).collect();
    entries.dedup();
    assert_eq!(entries.len(), MODES.len());
    for mode in MODES {
        let source = effect(mode, "").unwrap();
        assert_eq!(source.matches("@fragment").count(), 1, "{mode:?}");
        assert!(source.contains(fragment_entry(mode).unwrap()), "{mode:?}");
    }
}

#[test]
fn a_rectangle_shader_carries_the_clip_function_once() {
    for mode in [ShaderMode::PAINT, ShaderMode::COVERAGE] {
        let source = effect(mode, "").unwrap();
        assert_eq!(
            source.matches("fn cr_clip_coverage(").count(),
            1,
            "{mode:?}"
        );
    }
    let filter = effect(ShaderMode::FILTER, "").unwrap();
    assert_eq!(filter.matches("fn cr_clip_coverage(").count(), 0);
}

#[test]
fn the_clip_function_reads_the_kept_part_only_behind_its_test() {
    let source = part("sdf");
    let start = source.find("fn cr_clip_coverage(").unwrap();
    let body = &source[start..];
    let body = &body[..body.find("\n}\n").unwrap()];
    let branch = body.find("if scalars.y != cr_NO_SPACE").unwrap();
    for field in ["cr_clip_kept_aabb", "cr_load_spatial"] {
        assert!(
            body.find(field).unwrap() > branch,
            "`{field}` is read before the test"
        );
    }
}

#[test]
fn no_mode_declares_two_things_at_one_binding() {
    for mode in MODES {
        let source = effect(mode, "").unwrap();
        let mut seen: Vec<&str> = source
            .match_indices("@group(")
            .map(|(at, _)| {
                let tail = &source[at..];
                &tail[..tail.find("var").unwrap_or(tail.len())]
            })
            .collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), before, "{mode:?} binds one slot twice");
    }
}
