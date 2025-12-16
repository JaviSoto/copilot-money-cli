use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;

use graphql_parser::query::{
    Definition, Document, FragmentDefinition, OperationDefinition, Selection, SelectionSet, Type,
    TypeCondition,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeRef {
    Named(String),
    List(Box<TypeRef>),
    NonNull(Box<TypeRef>),
}

impl TypeRef {
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub ty: TypeRef,
}

#[derive(Debug, Default)]
pub struct SchemaDraft {
    pub objects: BTreeMap<String, BTreeMap<String, FieldDef>>,
    pub inputs: BTreeSet<String>,
    pub unions: BTreeMap<String, BTreeSet<String>>,
    pub scalars: BTreeSet<String>,
}

impl SchemaDraft {
    pub fn ensure_object(&mut self, name: &str) {
        self.objects.entry(name.to_string()).or_default();
    }

    pub fn add_field(&mut self, object: &str, field_name: &str, ty: TypeRef) {
        self.ensure_object(object);
        self.objects
            .entry(object.to_string())
            .or_default()
            .entry(field_name.to_string())
            .or_insert_with(|| FieldDef {
                name: field_name.to_string(),
                ty,
            });
    }
}

pub fn render_schema_from_operations(graphql_files: &[PathBuf]) -> anyhow::Result<String> {
    let mut sources = Vec::new();
    for p in graphql_files {
        sources.push((p.clone(), fs::read_to_string(p)?));
    }

    let mut docs = Vec::new();
    for (path, src) in &sources {
        let doc: Document<String> = graphql_parser::parse_query(src)
            .map_err(|e| anyhow::anyhow!("parse error in {}: {e}", path.display()))?;
        docs.push((path.clone(), doc));
    }

    let mut fragments: HashMap<String, FragmentDefinition<String>> = HashMap::new();
    let mut operations: Vec<OperationDefinition<String>> = Vec::new();
    for (_, doc) in &docs {
        for def in &doc.definitions {
            match def {
                Definition::Fragment(frag) => {
                    fragments.insert(frag.name.clone(), frag.clone());
                }
                Definition::Operation(op) => {
                    operations.push(op.clone());
                }
            }
        }
    }

    let mut draft = SchemaDraft::default();
    draft.scalars.insert("JSON".to_string());
    draft.ensure_object("Query");
    draft.ensure_object("Mutation");

    for op in &operations {
        match op {
            OperationDefinition::Query(q) => {
                collect_inputs_from_vars(&mut draft, &q.variable_definitions);
                process_selection_set(&mut draft, "Query", &q.selection_set, &fragments);
            }
            OperationDefinition::Mutation(m) => {
                collect_inputs_from_vars(&mut draft, &m.variable_definitions);
                process_selection_set(&mut draft, "Mutation", &m.selection_set, &fragments);
            }
            OperationDefinition::Subscription(s) => {
                collect_inputs_from_vars(&mut draft, &s.variable_definitions);
                process_selection_set(&mut draft, "Query", &s.selection_set, &fragments);
            }
            OperationDefinition::SelectionSet(ss) => {
                process_selection_set(&mut draft, "Query", ss, &fragments);
            }
        }
    }

    for frag in fragments.values() {
        let ty = type_condition_name(&frag.type_condition);
        draft.ensure_object(&ty);
        process_selection_set(&mut draft, &ty, &frag.selection_set, &fragments);
    }

    Ok(render_schema(&draft, &sources))
}

fn render_schema(draft: &SchemaDraft, sources: &[(PathBuf, String)]) -> String {
    let mut out = String::new();
    out.push_str("# Generated stub schema (best-effort)\n");
    out.push_str("# Source docs:\n");
    for (p, _) in sources {
        out.push_str(&format!("# - {}\n", p.display()));
    }
    out.push('\n');

    for scalar in &draft.scalars {
        out.push_str(&format!("scalar {scalar}\n"));
    }
    out.push('\n');

    out.push_str("schema { query: Query }\n\n");

    for (union_name, members) in &draft.unions {
        let rhs = members.iter().cloned().collect::<Vec<_>>().join(" | ");
        out.push_str(&format!("union {union_name} = {rhs}\n\n"));
    }

    for (type_name, fields) in &draft.objects {
        if type_name == "Mutation" {
            // Don’t emit empty Mutation; it confuses some tools unless referenced by schema.
            if fields.is_empty() {
                continue;
            }
        }

        out.push_str(&format!("type {type_name} {{\n"));
        if fields.is_empty() {
            out.push_str("  _placeholder: JSON\n");
        } else {
            for (field_name, field) in fields {
                out.push_str(&format!("  {field_name}: {}\n", render_type_ref(&field.ty)));
            }
        }
        out.push_str("}\n\n");
    }

    for input_name in &draft.inputs {
        out.push_str(&format!("input {input_name} {{\n  _stub: JSON\n}}\n\n"));
    }

    out
}

fn render_type_ref(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Named(n) => n.clone(),
        TypeRef::List(inner) => format!("[{}]", render_type_ref(inner)),
        TypeRef::NonNull(inner) => format!("{}!", render_type_ref(inner)),
    }
}

fn collect_inputs_from_vars(
    draft: &mut SchemaDraft,
    vars: &[graphql_parser::query::VariableDefinition<String>],
) {
    for v in vars {
        collect_named_types_from_var_type(&v.var_type, &mut draft.inputs, &mut draft.scalars);
    }
}

fn collect_named_types_from_var_type(
    ty: &Type<String>,
    inputs: &mut BTreeSet<String>,
    scalars: &mut BTreeSet<String>,
) {
    match ty {
        Type::NamedType(name) => {
            if is_builtin_scalar(name) {
                return;
            }
            // We don't know if it's an input object or scalar; treat as input.
            inputs.insert(name.clone());
            scalars.insert("JSON".to_string());
        }
        Type::ListType(inner) => collect_named_types_from_var_type(inner, inputs, scalars),
        Type::NonNullType(inner) => collect_named_types_from_var_type(inner, inputs, scalars),
    }
}

fn is_builtin_scalar(name: &str) -> bool {
    matches!(name, "String" | "Int" | "Float" | "Boolean" | "ID")
}

fn process_selection_set(
    draft: &mut SchemaDraft,
    current_type: &str,
    selection_set: &SelectionSet<String>,
    fragments: &HashMap<String, FragmentDefinition<String>>,
) {
    if draft.unions.contains_key(current_type) {
        process_union_selection_set(draft, selection_set, fragments);
        return;
    }

    draft.ensure_object(current_type);

    for selection in &selection_set.items {
        match selection {
            Selection::Field(field) => {
                if field.name == "__typename" {
                    continue;
                }

                if field.selection_set.items.is_empty() {
                    let ty = infer_leaf_scalar(&field.name);
                    draft.add_field(current_type, &field.name, ty);
                } else {
                    let inferred = infer_output_type_for_field(
                        draft,
                        current_type,
                        &field.name,
                        &field.selection_set,
                        fragments,
                    );
                    draft.add_field(current_type, &field.name, TypeRef::named(inferred.clone()));
                    if draft.unions.contains_key(&inferred) {
                        process_union_selection_set(draft, &field.selection_set, fragments);
                    } else {
                        process_selection_set(draft, &inferred, &field.selection_set, fragments);
                    }
                }
            }
            Selection::FragmentSpread(spread) => {
                if let Some(frag) = fragments.get(&spread.fragment_name) {
                    // Expand fragment fields into the current type.
                    process_selection_set(draft, current_type, &frag.selection_set, fragments);
                    // Also ensure the fragment's declared type exists.
                    let ty = type_condition_name(&frag.type_condition);
                    draft.ensure_object(&ty);
                    process_selection_set(draft, &ty, &frag.selection_set, fragments);
                }
            }
            Selection::InlineFragment(inline) => {
                let ty = inline
                    .type_condition
                    .as_ref()
                    .map(type_condition_name)
                    .unwrap_or_else(|| current_type.to_string());
                process_selection_set(draft, &ty, &inline.selection_set, fragments);
            }
        }
    }
}

fn process_union_selection_set(
    draft: &mut SchemaDraft,
    selection_set: &SelectionSet<String>,
    fragments: &HashMap<String, FragmentDefinition<String>>,
) {
    for selection in &selection_set.items {
        match selection {
            Selection::InlineFragment(inline) => {
                if let Some(tc) = &inline.type_condition {
                    let ty = type_condition_name(tc);
                    draft.ensure_object(&ty);
                    process_selection_set(draft, &ty, &inline.selection_set, fragments);
                }
            }
            Selection::FragmentSpread(spread) => {
                if let Some(frag) = fragments.get(&spread.fragment_name) {
                    let ty = type_condition_name(&frag.type_condition);
                    draft.ensure_object(&ty);
                    process_selection_set(draft, &ty, &frag.selection_set, fragments);
                }
            }
            Selection::Field(_) => {}
        }
    }
}

fn infer_leaf_scalar(field_name: &str) -> TypeRef {
    if field_name == "id" || field_name.ends_with("Id") || field_name.ends_with("ID") {
        return TypeRef::named("ID");
    }
    TypeRef::named("JSON")
}

fn infer_output_type_for_field(
    draft: &mut SchemaDraft,
    parent: &str,
    field_name: &str,
    selection_set: &SelectionSet<String>,
    fragments: &HashMap<String, FragmentDefinition<String>>,
) -> String {
    let mut fragment_type_conditions = BTreeSet::<String>::new();
    let mut inline_type_conditions = BTreeSet::<String>::new();

    for sel in &selection_set.items {
        match sel {
            Selection::FragmentSpread(spread) => {
                if let Some(frag) = fragments.get(&spread.fragment_name) {
                    fragment_type_conditions.insert(type_condition_name(&frag.type_condition));
                }
            }
            Selection::InlineFragment(inline) => {
                if let Some(tc) = &inline.type_condition {
                    inline_type_conditions.insert(type_condition_name(tc));
                }
            }
            Selection::Field(_) => {}
        }
    }

    if !inline_type_conditions.is_empty() {
        let union_name = format!("{parent}{}Union", pascal_case(field_name));
        for m in &inline_type_conditions {
            draft.ensure_object(m);
        }
        draft
            .unions
            .entry(union_name.clone())
            .or_default()
            .extend(inline_type_conditions);
        return union_name;
    }

    if fragment_type_conditions.len() == 1 {
        return fragment_type_conditions.into_iter().next().unwrap();
    }

    format!("{parent}{}", pascal_case(field_name))
}

fn pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' || ch == ' ' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn type_condition_name(tc: &TypeCondition<String>) -> String {
    match tc {
        TypeCondition::On(name) => name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_contains_fragment_type_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("a.graphql");
        fs::write(
            &p,
            r#"
query Q { thing { ...ThingFields } }
fragment ThingFields on Thing { id name }
"#,
        )
        .unwrap();

        let rendered = render_schema_from_operations(&[p]).unwrap();
        assert!(rendered.contains("type Thing"));
        assert!(rendered.contains("id: ID"));
        assert!(rendered.contains("name: JSON"));
    }
}
