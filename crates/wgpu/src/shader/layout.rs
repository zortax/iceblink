//! The memory layout WGSL gives the structures of a module, read from its text.

use std::collections::BTreeMap;

/// One scalar, vector or matrix leaf of a structure, named by its path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) offset: usize,
    pub(crate) size: usize,
}

/// A structure's leaves, size and alignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    pub(crate) fields: Vec<Field>,
    pub(crate) size: usize,
    pub(crate) align: usize,
}

/// Returns the layout of every structure `source` declares whose members are scalars, vectors,
/// matrices or structures declared before it.
pub(crate) fn parse(source: &str) -> BTreeMap<String, Layout> {
    let mut layouts: BTreeMap<String, Layout> = BTreeMap::new();
    let mut rest = source;
    while let Some(at) = find_declaration(rest) {
        rest = &rest[at + "struct".len()..];
        let Some((name, body, tail)) = split_declaration(rest) else {
            break;
        };
        rest = tail;
        if let Some(layout) = layout_of(&body, &layouts) {
            layouts.insert(name, layout);
        }
    }
    layouts
}

/// Returns where the next `struct` keyword of `source` starts.
fn find_declaration(source: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(at) = source[from..].find("struct") {
        let at = from + at;
        let before = source[..at].chars().next_back();
        let after = source[at + "struct".len()..].chars().next();
        let boundary = before.is_none_or(|character| !character.is_alphanumeric())
            && after.is_some_and(char::is_whitespace);
        if boundary {
            return Some(at);
        }
        from = at + "struct".len();
    }
    None
}

/// Splits the text after a `struct` keyword into the name, the body and the text after it.
fn split_declaration(source: &str) -> Option<(String, String, &str)> {
    let open = source.find('{')?;
    let close = source[open..].find('}')? + open;
    let name = source[..open].trim().to_owned();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some((
        name,
        source[open + 1..close].to_owned(),
        &source[close + 1..],
    ))
}

/// Returns the layout of a structure `body`, or `None` when a member is not a name and a type that
/// is a scalar, a vector, a matrix or one of `known`.
fn layout_of(body: &str, known: &BTreeMap<String, Layout>) -> Option<Layout> {
    let mut fields = Vec::new();
    let mut offset = 0usize;
    let mut align = 1usize;
    // A comment may hold a comma, which would split one member in two.
    let body = strip_comments(body);
    for member in body.split(',') {
        let member = member.trim();
        if member.is_empty() {
            continue;
        }
        let (name, type_name) = member.split_once(':')?;
        let name = name.trim();
        let type_name = type_name.trim();
        let (member_size, member_align, leaves) = describe(type_name, known)?;
        offset = round_up(offset, member_align);
        align = align.max(member_align);
        match leaves {
            None => fields.push(Field {
                name: name.to_owned(),
                offset,
                size: member_size,
            }),
            Some(nested) => {
                for leaf in nested {
                    fields.push(Field {
                        name: format!("{name}.{}", leaf.name),
                        offset: offset + leaf.offset,
                        size: leaf.size,
                    });
                }
            }
        }
        offset += member_size;
    }
    Some(Layout {
        size: round_up(offset, align),
        align,
        fields,
    })
}

/// Returns the size, alignment and nested leaves of the type `type_name`.
fn describe(
    type_name: &str,
    known: &BTreeMap<String, Layout>,
) -> Option<(usize, usize, Option<Vec<Field>>)> {
    let scalar = matches!(type_name, "f32" | "u32" | "i32");
    if scalar {
        return Some((4, 4, None));
    }
    if let Some(inner) = vector_arity(type_name) {
        let size = 4 * inner;
        let align = if inner == 3 { 16 } else { size };
        return Some((size, align, None));
    }
    if let Some((columns, rows)) = matrix_shape(type_name) {
        let align = if rows == 3 { 16 } else { 4 * rows };
        return Some((round_up(4 * rows, align) * columns, align, None));
    }
    let structure = known.get(type_name)?;
    Some((
        structure.size,
        structure.align,
        Some(structure.fields.clone()),
    ))
}

fn vector_arity(type_name: &str) -> Option<usize> {
    let rest = type_name.strip_prefix("vec")?;
    let (arity, _) = rest.split_once('<')?;
    match arity {
        "2" => Some(2),
        "3" => Some(3),
        "4" => Some(4),
        _ => None,
    }
}

fn matrix_shape(type_name: &str) -> Option<(usize, usize)> {
    let rest = type_name.strip_prefix("mat")?;
    let (shape, _) = rest.split_once('<')?;
    let (columns, rows) = shape.split_once('x')?;
    let arity = |text: &str| match text {
        "2" => Some(2),
        "3" => Some(3),
        "4" => Some(4),
        _ => None,
    };
    Some((arity(columns)?, arity(rows)?))
}

/// Returns `text` without its line comments.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn round_up(value: usize, align: usize) -> usize {
    value.div_ceil(align.max(1)) * align.max(1)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn a_structure_of_scalars_is_packed() {
        let layouts = parse("struct Bounds { x: f32, y: f32, w: f32, h: f32 }");
        let bounds = &layouts["Bounds"];
        assert_eq!(bounds.size, 16);
        assert_eq!(bounds.align, 4);
        assert_eq!(bounds.fields[3].offset, 12);
    }

    #[test]
    fn a_matrix_is_as_many_padded_columns_as_it_has() {
        let layouts = parse("struct Spatial { matrix: mat4x4<f32> }");
        let spatial = &layouts["Spatial"];
        assert_eq!(spatial.size, 64, "four columns of four floats");
        assert_eq!(
            spatial.align, 16,
            "aligned like the column vector it is made of"
        );
        assert_eq!(
            spatial.fields.len(),
            1,
            "a matrix is one member, not sixteen"
        );
    }

    #[test]
    fn a_matrix_of_three_row_columns_strides_past_its_own_numbers() {
        let layouts = parse("struct Skew { m: mat3x3<f32> }");
        assert_eq!(
            layouts["Skew"].size, 48,
            "nine floats, but three columns that each stride sixteen bytes",
        );
    }

    #[test]
    fn a_nested_structure_is_flattened_into_its_leaves() {
        let source = "
            struct Bounds { x: f32, y: f32, w: f32, h: f32 }
            struct Quad { order: u32, style: u32, bounds: Bounds, clip: u32 }
        ";
        let layouts = parse(source);
        let quad = &layouts["Quad"];
        assert_eq!(quad.size, 28);
        let names: Vec<&str> = quad
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "order", "style", "bounds.x", "bounds.y", "bounds.w", "bounds.h", "clip"
            ]
        );
        assert_eq!(quad.fields[2].offset, 8);
        assert_eq!(quad.fields[6].offset, 24);
    }

    #[test]
    fn a_vector_carries_its_own_alignment_and_moves_what_follows_it() {
        // A `vec4` after two scalars does not begin where a packed Rust `[f32; 4]` would.
        let layouts = parse("struct Padded { a: u32, b: u32, c: vec4<f32> }");
        let padded = &layouts["Padded"];
        assert_eq!(padded.align, 16);
        assert_eq!(padded.fields[2].offset, 16);
        assert_eq!(padded.size, 32);
    }

    #[test]
    fn a_trailing_comma_and_a_comment_are_not_fields() {
        let layouts = parse(
            "struct Thing {
                // The first one.
                first: f32,
                second: u32, // and the second
            }",
        );
        assert_eq!(layouts["Thing"].fields.len(), 2);
    }

    #[test]
    fn a_word_ending_in_struct_does_not_begin_a_declaration() {
        let layouts = parse("fn destruct() -> f32 { return 1.0; }\nstruct Real { a: f32 }");
        assert_eq!(layouts.len(), 1);
        assert!(layouts.contains_key("Real"));
    }
}
