#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_derived_type_indexer_with_generic_constraints() {
    let mut t = TestingT;
    run_test_derived_type_indexer_with_generic_constraints(&mut t);
}

fn run_test_derived_type_indexer_with_generic_constraints(t: &mut TestingT) {
    if should_skip_if_failing("TestDerivedTypeIndexerWithGenericConstraints") {
        return;
    }
    let content = r"// @strict: false
class CollectionItem {
    x: number;
}
class Entity extends CollectionItem {
    y: number;
}
class BaseCollection<TItem extends CollectionItem>  {
    _itemsByKey: { [key: string]: TItem; };
}
class DbSet<TEntity extends Entity> extends BaseCollection<TEntity> { // error
    _itemsByKey: { [key: string]: TEntity; } = {};
}
var a: BaseCollection<CollectionItem>;
var /**/r = a._itemsByKey['x']; // should just say CollectionItem not TItem extends CollectionItem
var result = r.x;
a = new DbSet<Entity>();
var r2 = a._itemsByKey['x'];
var result2 = r2.x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var r: CollectionItem", "");
    f.verify_no_errors();
    done();
}
