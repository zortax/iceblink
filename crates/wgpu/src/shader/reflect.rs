//! Comparing a module's structures with the Rust structures the renderer writes.

use crate::shader::layout::{Layout, parse};

/// One member of a Rust structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Member {
    pub(crate) name: &'static str,
    pub(crate) offset: usize,
    pub(crate) size: usize,
}

/// Describes a Rust structure for [`check`], as the WGSL structure name and the member paths.
///
/// A member named by a path, such as `shadow.blur`, compares with the WGSL member of its last
/// name. A member whose name starts with `_` is padding and widens the member before it.
macro_rules! reflected {
    ($type:ty, $wgsl:literal, [$($($field:ident).+),+ $(,)?]) => {
        $crate::shader::reflect::Reflected {
            name: $wgsl,
            size: ::core::mem::size_of::<$type>(),
            members: vec![$(
                $crate::shader::reflect::Member {
                    name: $crate::shader::reflect::last_name(stringify!($($field).+)),
                    offset: ::core::mem::offset_of!($type, $($field).+),
                    size: ::core::mem::size_of_val(
                        &<$type as ::bytemuck::Zeroable>::zeroed().$($field).+,
                    ),
                },
            )+],
        }
    };
}

pub(crate) use reflected;

/// Returns the last name of a member path.
pub(crate) fn last_name(path: &'static str) -> &'static str {
    path.rsplit('.').next().unwrap_or(path).trim()
}

/// A Rust structure as [`check`] compares it.
pub(crate) struct Reflected {
    pub(crate) name: &'static str,
    pub(crate) size: usize,
    pub(crate) members: Vec<Member>,
}

/// Compares every structure of `structures` with its declaration in `source`.
///
/// Fails with a description of the first difference, or of a structure `source` does not
/// declare.
pub(crate) fn check(source: &str, structures: &[Reflected]) -> Result<(), String> {
    let layouts = parse(source);
    for structure in structures {
        let Some(layout) = layouts.get(structure.name) else {
            return Err(format!(
                "the shader declares no `{}`, so nothing was compared against it",
                structure.name
            ));
        };
        compare(structure, layout)?;
    }
    Ok(())
}

/// Compares one Rust structure with its WGSL `layout`.
fn compare(structure: &Reflected, layout: &Layout) -> Result<(), String> {
    let members = merged(layout).map_err(|error| format!("`{}`: {error}", structure.name))?;
    let rust = padded(&structure.members);
    let structure = &Reflected {
        name: structure.name,
        size: structure.size,
        members: rust,
    };
    if layout.size != structure.size {
        return Err(format!(
            "`{}` is {} bytes in the shader and {} in Rust",
            structure.name, layout.size, structure.size
        ));
    }
    if members.len() != structure.members.len() {
        return Err(format!(
            "`{}` has {} members in the shader and {} in Rust",
            structure.name,
            members.len(),
            structure.members.len()
        ));
    }
    for (shader, rust) in members.iter().zip(&structure.members) {
        if shader.name != rust.name {
            return Err(format!(
                "`{}` member {} is `{}` in the shader and `{}` in Rust",
                structure.name, rust.offset, shader.name, rust.name
            ));
        }
        if shader.offset != rust.offset || shader.size != rust.size {
            return Err(format!(
                "`{}.{}` is {} bytes at offset {} in the shader and {} bytes at offset {} in Rust",
                structure.name, rust.name, shader.size, shader.offset, rust.size, rust.offset
            ));
        }
    }
    Ok(())
}

/// Returns `members` with each padding member merged into the member before it.
// The shader reads a member and its padding as one word.
fn padded(members: &[Member]) -> Vec<Member> {
    let mut out: Vec<Member> = Vec::with_capacity(members.len());
    for member in members {
        match out.last_mut() {
            Some(last) if member.name.starts_with('_') => last.size += member.size,
            _ => out.push(*member),
        }
    }
    out
}

/// One member of a WGSL structure, with its nested leaves merged.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Merged {
    name: String,
    offset: usize,
    size: usize,
}

/// Returns the members of `layout`, each with its nested leaves and the padding member after it
/// merged.
///
/// Fails when a member is preceded by padding.
fn merged(layout: &Layout) -> Result<Vec<Merged>, String> {
    let mut members: Vec<Merged> = Vec::new();
    for field in &layout.fields {
        let (head, _) = field.name.split_once('.').unwrap_or((&field.name, ""));
        match members.last_mut() {
            Some(last) if last.name == head || head.starts_with('_') => {
                if last.offset + last.size != field.offset {
                    return Err(format!("`{}` has padding before `{}`", head, field.name));
                }
                last.size += field.size;
            }
            _ => {
                if let Some(last) = members.last()
                    && last.offset + last.size != field.offset
                {
                    return Err(format!("has padding before `{}`", field.name));
                }
                members.push(Merged {
                    name: head.to_owned(),
                    offset: field.offset,
                    size: field.size,
                });
            }
        }
    }
    Ok(members)
}
