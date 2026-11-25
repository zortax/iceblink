//! Checking that every loader reads each field of its record from the word the field is stored at.
//!
//! A loader is a `fn load_*` or `fn cr_load_*` of a module, or `cr_clip_rounded`. Its reads are
//! taken from its text and compared with the scalars of the structure it returns, in order.

use wgpu::naga;

/// The type of one four-byte word of a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Word {
    Uint,
    Sint,
    Float,
}

/// Appends the word of each scalar of `ty`, in memory order.
///
/// Fails for a type with padding, with scalars that are not four bytes, or that a loader cannot
/// produce.
fn scalars(
    module: &naga::Module,
    ty: naga::Handle<naga::Type>,
    into: &mut Vec<Word>,
) -> Result<(), String> {
    let word = |scalar: naga::Scalar| match (scalar.kind, scalar.width) {
        (naga::ScalarKind::Uint, 4) => Ok(Word::Uint),
        (naga::ScalarKind::Sint, 4) => Ok(Word::Sint),
        (naga::ScalarKind::Float, 4) => Ok(Word::Float),
        other => Err(format!("a scalar that is not four bytes: {other:?}")),
    };
    match &module.types[ty].inner {
        naga::TypeInner::Scalar(scalar) => into.push(word(*scalar)?),
        naga::TypeInner::Vector { size, scalar } => {
            into.extend(std::iter::repeat_n(word(*scalar)?, *size as usize));
        }
        naga::TypeInner::Matrix {
            columns,
            rows,
            scalar,
        } => {
            // A column of three is padded to four, and a loader has no texel for the padding.
            if *rows != naga::VectorSize::Quad {
                return Err("a matrix whose columns are padded".to_owned());
            }
            into.extend(std::iter::repeat_n(
                word(*scalar)?,
                *columns as usize * *rows as usize,
            ));
        }
        naga::TypeInner::Struct { members, span } => {
            let start = into.len();
            for member in members {
                if member.offset as usize != (into.len() - start) * 4 {
                    return Err(format!("padding before `{:?}`", member.name));
                }
                scalars(module, member.ty, into)?;
            }
            if *span as usize != (into.len() - start) * 4 {
                return Err("padding at the end of a structure".to_owned());
            }
        }
        other => return Err(format!("a type a loader cannot produce: {other:?}")),
    }
    Ok(())
}

/// Returns the texel offset added in the `cr_table_texel(` call at `at` in `text`, or zero when
/// none is added.
fn texel_of(text: &str, at: usize) -> u32 {
    let argument = &text[at + "cr_table_texel(".len()..];
    let argument = &argument[..argument.find(')').expect("the call closes")];
    argument
        .rsplit_once("+ ")
        .and_then(|(_, texel)| texel.trim_end_matches('u').parse().ok())
        .unwrap_or(0)
}

/// Returns the word index and type of every word the loader `body` reads into its record, in the
/// order the returned expression lists them.
fn words_read(body: &str) -> Vec<(u32, Word)> {
    let returned = &body[body.find("return ").expect("a loader returns its record")..];
    let mut read = inline_reads(returned);
    read.extend(named_reads(returned, &named_texels(body)));
    // Stable, so the four words of one converted texel keep their order.
    read.sort_by_key(|(at, ..)| *at);
    read.into_iter()
        .map(|(_, index, word)| (index, word))
        .collect()
}

/// Returns the texels `body` loads into a named binding, with the name, the texel and whether the
/// texel is read as floats.
fn named_texels(body: &str) -> Vec<(&str, u32, bool)> {
    // Each `let name = textureLoad(...)` is one texel, already floats under `cr_texel_f32`.
    let mut texels = Vec::new();
    for line in body.lines().map(str::trim) {
        let Some(rest) = line.strip_prefix("let ") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(" = ") else {
            continue;
        };
        if let Some(at) = value.find("cr_table_texel(") {
            texels.push((
                name,
                texel_of(value, at),
                value.starts_with("cr_texel_f32("),
            ));
        }
    }
    texels
}

/// Returns the words of the texels the expression `returned` loads in place, with their position
/// in `returned`.
fn inline_reads(returned: &str) -> Vec<(usize, u32, Word)> {
    let mut read = Vec::new();
    // A texel loaded where it is passed is four words: floats under `cr_texel_f32`, else as read.
    for (at, _) in returned.match_indices("textureLoad(") {
        let floats = returned[..at].ends_with("cr_texel_f32(");
        let (word, close) = if floats {
            (Word::Float, ", 0))")
        } else {
            (Word::Uint, ", 0)")
        };
        let call = at
            + returned[at..]
                .find("cr_table_texel(")
                .expect("a load names a texel");
        let texel = texel_of(returned, call);
        // All four channels, unless a swizzle after the call keeps some of them.
        let after = &returned[call..];
        let after = &after[after.find(close).expect("the load closes") + close.len()..];
        let kept: Vec<u32> = match after.strip_prefix('.') {
            Some(swizzle) => swizzle
                .chars()
                .map_while(|letter| "xyzw".find(letter).map(|channel| channel as u32))
                .collect(),
            None => (0..4).collect(),
        };
        read.extend(
            kept.into_iter()
                .map(|channel| (at, texel * 4 + channel, word)),
        );
    }
    read
}

/// Returns the words of the named `texels` the expression `returned` reads component by component,
/// with their position in `returned`.
fn named_reads(returned: &str, texels: &[(&str, u32, bool)]) -> Vec<(usize, u32, Word)> {
    let mut read = Vec::new();
    for &(name, texel, floats) in texels {
        for (component, letter) in ["x", "y", "z", "w"].into_iter().enumerate() {
            let field = format!("{name}.{letter}");
            for (at, _) in returned.match_indices(&field) {
                let whole_name = returned[..at]
                    .chars()
                    .next_back()
                    .is_none_or(|before| !before.is_alphanumeric() && before != '_');
                let whole_field = returned[at + field.len()..]
                    .chars()
                    .next()
                    .is_none_or(|after| !after.is_alphanumeric() && after != '_');
                if !whole_name || !whole_field {
                    continue;
                }
                let word = if returned[..at].ends_with("bitcast<f32>(") {
                    Word::Float
                } else if returned[..at].ends_with("bitcast<i32>(") {
                    Word::Sint
                } else if floats {
                    Word::Float
                } else {
                    Word::Uint
                };
                read.push((at, texel * 4 + component as u32, word));
            }
        }
    }
    read
}

/// Checks every loader of `source` against the record it returns in the compiled `module`, and
/// returns how many it checked.
///
/// Fails with a description of the first loader that is missing from `module`, returns nothing,
/// returns a record a loader cannot fill, reads other words than its record holds, or steps by
/// another stride than its record's.
pub(crate) fn check_loaders(source: &str, module: &naga::Module) -> Result<usize, String> {
    let loaders = source
        .match_indices("\nfn load_")
        .chain(source.match_indices("\nfn cr_load_"))
        .chain(source.match_indices("\nfn cr_clip_rounded("));
    let mut checked = 0;
    for (at, _) in loaders {
        let text = &source[at + 1..];
        check_loader(
            &text[..text.find("\n}").expect("a function closes")],
            module,
        )?;
        checked += 1;
    }
    Ok(checked)
}

/// Checks one loader, whose text runs from its `fn` to its last line.
fn check_loader(text: &str, module: &naga::Module) -> Result<(), String> {
    let name = &text["fn ".len()..text.find('(').expect("a function has parameters")];
    let (_, function) = module
        .functions
        .iter()
        .find(|(_, function)| function.name.as_deref() == Some(name))
        .ok_or_else(|| format!("`{name}` is not in the compiled module"))?;
    let result = function
        .result
        .as_ref()
        .ok_or_else(|| format!("`{name}` returns nothing"))?;
    let mut expected = Vec::new();
    scalars(module, result.ty, &mut expected).map_err(|why| format!("`{name}`: {why}"))?;

    let read = words_read(text);
    let wanted: Vec<(u32, Word)> = expected
        .iter()
        .enumerate()
        .map(|(index, word)| (index as u32, *word))
        .collect();
    if read != wanted {
        let first = read
            .iter()
            .zip(&wanted)
            .position(|(read, wanted)| read != wanted)
            .unwrap_or(read.len().min(wanted.len()));
        return Err(format!(
            "`{name}` reads {} words and its record has {}; they first differ at word \
             {first}: read {:?}, declared {:?}",
            read.len(),
            wanted.len(),
            read.get(first),
            wanted.get(first),
        ));
    }

    let texels = expected.len().div_ceil(4);
    match stride(text) {
        Some(stride) if stride != texels => Err(format!(
            "`{name}` steps by {stride} texels and its record is {texels}"
        )),
        _ => Ok(()),
    }
}

/// Returns the stride in texels that the loader `text` steps through its table by, if it states
/// one.
fn stride(text: &str) -> Option<usize> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("let base = "))
        .and_then(|base| base.rsplit_once("* "))
        .and_then(|(_, stride)| {
            stride
                .trim_end_matches(';')
                .trim_end_matches('u')
                .parse()
                .ok()
        })
}
