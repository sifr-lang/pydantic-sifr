use pydantic_sifr_core::{
    CollectionConstraints, ExtraPolicy, ModelField, ModelSchema, PreparedSchema, Schema, SchemaTag,
    SerializationPlan,
};
use sifr_runtime::interop::structural::{ShapeIdentity, primitive};

#[test]
fn serializer_plan_preserves_model_projection_order() {
    let schema = Schema::Model(
        ModelSchema::new(
            "PlanModel",
            primitive("tests.PlanModel"),
            vec![
                ModelField::required("name", Schema::String(Default::default())),
                ModelField::required(
                    "scores",
                    Schema::List {
                        item: Box::new(Schema::exact_integer()),
                        constraints: CollectionConstraints::default(),
                    },
                ),
            ],
            ExtraPolicy::Ignore,
            false,
            true,
        )
        .unwrap_or_else(|error| panic!("model schema failed: {error}")),
    );

    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let expected_identity: ShapeIdentity = prepared.structural_identity();
    let plan = SerializationPlan::from_prepared(prepared)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    assert_eq!(plan.structural_identity(), expected_identity);

    let root = plan
        .node(plan.root())
        .unwrap_or_else(|| panic!("serializer root is missing"));
    assert_eq!(root.tag(), SchemaTag::Model);
    assert_eq!(root.fields().len(), 2);
    assert_eq!(root.fields()[0].name(), "name");
    assert_eq!(root.fields()[1].name(), "scores");
    assert_eq!(
        plan.node(root.fields()[0].node())
            .unwrap_or_else(|| panic!("name serializer is missing"))
            .tag(),
        SchemaTag::String
    );
    assert_eq!(
        plan.node(root.fields()[1].node())
            .unwrap_or_else(|| panic!("scores serializer is missing"))
            .tag(),
        SchemaTag::List
    );
}

#[test]
fn serializer_plan_retains_control_and_collection_children() {
    let lax = Schema::List {
        item: Box::new(Schema::Bool),
        constraints: CollectionConstraints::default(),
    };
    let strict = Schema::List {
        item: Box::new(Schema::Bool),
        constraints: CollectionConstraints::default(),
    };
    let schema = Schema::lax_or_strict(lax, strict, false)
        .unwrap_or_else(|error| panic!("control schema failed: {error}"));
    let plan = PreparedSchema::new(&schema)
        .map_err(|error| error.to_string())
        .and_then(|prepared| {
            SerializationPlan::from_prepared(prepared).map_err(|error| error.to_string())
        })
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));

    let root = plan
        .node(plan.root())
        .unwrap_or_else(|| panic!("serializer root is missing"));
    assert_eq!(root.tag(), SchemaTag::LaxOrStrict);
    assert_eq!(root.children().len(), 2);
    for child in root.children() {
        let list = plan
            .node(*child)
            .unwrap_or_else(|| panic!("list serializer is missing"));
        assert_eq!(list.tag(), SchemaTag::List);
        assert_eq!(list.children().len(), 1);
        assert_eq!(
            plan.node(list.children()[0])
                .unwrap_or_else(|| panic!("bool serializer is missing"))
                .tag(),
            SchemaTag::Bool
        );
    }
}
