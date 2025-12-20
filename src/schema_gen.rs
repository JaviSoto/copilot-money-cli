use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;

use graphql_parser::query::{
    Definition, Document, FragmentDefinition, OperationDefinition, Selection, SelectionSet, Type,
    TypeCondition, Value, VariableDefinition,
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
    pub args: BTreeMap<String, TypeRef>,
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

        let fields = self.objects.entry(object.to_string()).or_default();
        match fields.get_mut(field_name) {
            Some(existing) => {
                if existing.ty != ty {
                    existing.ty = TypeRef::named("JSON");
                }
            }
            None => {
                fields.insert(
                    field_name.to_string(),
                    FieldDef {
                        name: field_name.to_string(),
                        ty,
                        args: BTreeMap::new(),
                    },
                );
            }
        }
    }

    pub fn add_field_arg(&mut self, object: &str, field_name: &str, arg_name: &str, ty: TypeRef) {
        self.ensure_object(object);
        let fields = self.objects.entry(object.to_string()).or_default();
        let field = fields
            .entry(field_name.to_string())
            .or_insert_with(|| FieldDef {
                name: field_name.to_string(),
                ty: TypeRef::named("JSON"),
                args: BTreeMap::new(),
            });

        match field.args.get_mut(arg_name) {
            Some(existing) => {
                if *existing != ty {
                    *existing = TypeRef::named("JSON");
                }
            }
            None => {
                field.args.insert(arg_name.to_string(), ty);
            }
        }
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
                let var_types = var_type_map(&q.variable_definitions);
                process_selection_set(
                    &mut draft,
                    "Query",
                    &q.selection_set,
                    &fragments,
                    &var_types,
                );
            }
            OperationDefinition::Mutation(m) => {
                collect_inputs_from_vars(&mut draft, &m.variable_definitions);
                let var_types = var_type_map(&m.variable_definitions);
                process_selection_set(
                    &mut draft,
                    "Mutation",
                    &m.selection_set,
                    &fragments,
                    &var_types,
                );
            }
            OperationDefinition::Subscription(s) => {
                collect_inputs_from_vars(&mut draft, &s.variable_definitions);
                let var_types = var_type_map(&s.variable_definitions);
                process_selection_set(
                    &mut draft,
                    "Query",
                    &s.selection_set,
                    &fragments,
                    &var_types,
                );
            }
            OperationDefinition::SelectionSet(ss) => {
                process_selection_set(&mut draft, "Query", ss, &fragments, &HashMap::new());
            }
        }
    }

    for frag in fragments.values() {
        let ty = type_condition_name(&frag.type_condition);
        draft.ensure_object(&ty);
        process_selection_set(
            &mut draft,
            &ty,
            &frag.selection_set,
            &fragments,
            &HashMap::new(),
        );
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

    let has_mutation = draft
        .objects
        .get("Mutation")
        .map(|m| !m.is_empty())
        .unwrap_or(false);
    if has_mutation {
        out.push_str("schema { query: Query mutation: Mutation }\n\n");
    } else {
        out.push_str("schema { query: Query }\n\n");
    }

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
                let args = render_args(&field.args);
                out.push_str(&format!(
                    "  {field_name}{args}: {}\n",
                    render_type_ref(&field.ty)
                ));
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

fn render_args(args: &BTreeMap<String, TypeRef>) -> String {
    if args.is_empty() {
        return String::new();
    }
    let parts = args
        .iter()
        .map(|(k, v)| format!("{k}: {}", render_type_ref(v)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("({parts})")
}

fn collect_inputs_from_vars(draft: &mut SchemaDraft, vars: &[VariableDefinition<String>]) {
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
    var_types: &HashMap<String, TypeRef>,
) {
    if draft.unions.contains_key(current_type) {
        process_union_selection_set(draft, selection_set, fragments, var_types);
        return;
    }

    draft.ensure_object(current_type);

    for selection in &selection_set.items {
        match selection {
            Selection::Field(field) => {
                if field.name == "__typename" {
                    continue;
                }

                // Capture argument names and best-effort types.
                for (arg_name, value) in &field.arguments {
                    if let Some(arg_ty) = infer_argument_type(value, var_types) {
                        draft.add_field_arg(current_type, &field.name, arg_name, arg_ty);
                    }
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
                        process_union_selection_set(
                            draft,
                            &field.selection_set,
                            fragments,
                            var_types,
                        );
                    } else {
                        process_selection_set(
                            draft,
                            &inferred,
                            &field.selection_set,
                            fragments,
                            var_types,
                        );
                    }
                }
            }
            Selection::FragmentSpread(spread) => {
                if let Some(frag) = fragments.get(&spread.fragment_name) {
                    // Expand fragment fields into the current type.
                    process_selection_set(
                        draft,
                        current_type,
                        &frag.selection_set,
                        fragments,
                        var_types,
                    );
                    // Also ensure the fragment's declared type exists.
                    let ty = type_condition_name(&frag.type_condition);
                    draft.ensure_object(&ty);
                    process_selection_set(draft, &ty, &frag.selection_set, fragments, var_types);
                }
            }
            Selection::InlineFragment(inline) => {
                let ty = inline
                    .type_condition
                    .as_ref()
                    .map(type_condition_name)
                    .unwrap_or_else(|| current_type.to_string());
                process_selection_set(draft, &ty, &inline.selection_set, fragments, var_types);
            }
        }
    }
}

fn process_union_selection_set(
    draft: &mut SchemaDraft,
    selection_set: &SelectionSet<String>,
    fragments: &HashMap<String, FragmentDefinition<String>>,
    var_types: &HashMap<String, TypeRef>,
) {
    for selection in &selection_set.items {
        match selection {
            Selection::InlineFragment(inline) => {
                if let Some(tc) = &inline.type_condition {
                    let ty = type_condition_name(tc);
                    draft.ensure_object(&ty);
                    process_selection_set(draft, &ty, &inline.selection_set, fragments, var_types);
                }
            }
            Selection::FragmentSpread(spread) => {
                if let Some(frag) = fragments.get(&spread.fragment_name) {
                    let ty = type_condition_name(&frag.type_condition);
                    draft.ensure_object(&ty);
                    process_selection_set(draft, &ty, &frag.selection_set, fragments, var_types);
                }
            }
            Selection::Field(_) => {}
        }
    }
}

fn infer_leaf_scalar(field_name: &str) -> TypeRef {
    if field_name == "id" {
        return TypeRef::NonNull(Box::new(TypeRef::named("ID")));
    }
    if field_name == "cursor" {
        return TypeRef::NonNull(Box::new(TypeRef::named("String")));
    }
    if field_name.ends_with("Id") || field_name.ends_with("ID") {
        return TypeRef::named("ID");
    }
    if field_name == "name" || field_name.ends_with("Name") {
        return TypeRef::named("String");
    }
    if field_name == "date" || field_name.ends_with("Date") {
        return TypeRef::named("String");
    }
    if field_name == "month" {
        return TypeRef::named("String");
    }
    if field_name.starts_with("is") {
        return TypeRef::named("Boolean");
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

fn var_type_map(vars: &[VariableDefinition<String>]) -> HashMap<String, TypeRef> {
    let mut out = HashMap::new();
    for v in vars {
        out.insert(v.name.clone(), type_ref_from_gql_type(&v.var_type));
    }
    out
}

fn type_ref_from_gql_type(ty: &Type<String>) -> TypeRef {
    match ty {
        Type::NamedType(n) => TypeRef::Named(n.clone()),
        Type::ListType(inner) => TypeRef::List(Box::new(type_ref_from_gql_type(inner))),
        Type::NonNullType(inner) => TypeRef::NonNull(Box::new(type_ref_from_gql_type(inner))),
    }
}

fn infer_argument_type(
    value: &Value<String>,
    var_types: &HashMap<String, TypeRef>,
) -> Option<TypeRef> {
    match value {
        Value::Variable(name) => var_types.get(name).cloned(),
        Value::Boolean(_) => Some(TypeRef::named("Boolean")),
        Value::Int(_) => Some(TypeRef::named("Int")),
        Value::Float(_) => Some(TypeRef::named("Float")),
        Value::String(_) => Some(TypeRef::named("String")),
        Value::Enum(_) => Some(TypeRef::named("JSON")),
        Value::List(_) => Some(TypeRef::named("JSON")),
        Value::Object(_) => Some(TypeRef::named("JSON")),
        Value::Null => Some(TypeRef::named("JSON")),
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
        assert!(rendered.contains("name: String"));
    }

    #[test]
    fn schema_renders_query_and_mutation() {
        let tmp = tempfile::tempdir().unwrap();
        let q = tmp.path().join("q.graphql");
        std::fs::write(
            &q,
            r#"query Q($first: Int!, $after: String) { transactions(first: $first, after: $after) { edges { node { id } } } }"#,
        )
        .unwrap();

        let m = tmp.path().join("m.graphql");
        std::fs::write(
            &m,
            r#"mutation M($id: ID!, $input: JSON) { deleteTag(id: $id) }"#,
        )
        .unwrap();

        let out = crate::schema_gen::render_schema_from_operations(&vec![q, m]).unwrap();
        assert!(out.contains("schema { query: Query mutation: Mutation }"));
        assert!(out.contains("type Query"));
        assert!(out.contains("type Mutation"));
        assert!(out.contains("deleteTag"));
    }

    #[test]
    fn schema_includes_fragment_type_condition_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("f.graphql");
        std::fs::write(
            &f,
            r#"fragment IconFields on EmojiUnicode { __typename unicode }"#,
        )
        .unwrap();

        let out = crate::schema_gen::render_schema_from_operations(&vec![f]).unwrap();
        assert!(out.contains("type EmojiUnicode"));
        assert!(out.contains("unicode"));
    }
}
