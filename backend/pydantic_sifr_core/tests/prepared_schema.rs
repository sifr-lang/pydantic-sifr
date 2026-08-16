use proptest::prelude::*;

use pydantic_sifr_core::{CollectionConstraints, PreparedSchema, Schema};

proptest! {
    #[test]
    fn arbitrary_prepared_schema_fields_never_panic(wrappers in prop::collection::vec(any::<bool>(), 0..300)) {
        let mut schema = Schema::Bool;
        for nullable in wrappers {
            schema = if nullable {
                Schema::Nullable(Box::new(schema))
            } else {
                Schema::List {
                    item: Box::new(schema),
                    constraints: CollectionConstraints::default(),
                }
            };
        }
        let _ = PreparedSchema::new(&schema);
    }
}
