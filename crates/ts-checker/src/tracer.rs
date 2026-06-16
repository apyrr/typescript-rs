use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use ts_ast as ast;
use ts_tracing as tracing;

use crate::checker::*;

// Tracer records types and trace events during type checking. A nil *Tracer
// is a valid no-op, so call sites can use `if tr := c.tracer; tr != nil` to
// gate work that only matters under --generateTrace.
pub struct Tracer<'a> {
    tracing: Rc<RefCell<&'a mut tracing::Tracing>>,
    recorder: RefCell<Box<dyn tracing::Tracer + 'a>>,
    checker_index: i32,
}

// NewTracer creates a Tracer for the given checker index that records both
// type-creation events and trace events through the provided tracing session.
pub fn new_tracer<'a>(tr: &'a mut tracing::Tracing, checker_index: i32) -> Tracer<'a> {
    let recorder = tr.new_type_tracer(checker_index);
    Tracer {
        tracing: Rc::new(RefCell::new(tr)),
        recorder: RefCell::new(Box::new(recorder)),
        checker_index,
    }
}

impl<'a> Tracer<'a> {
    pub fn record_type<'state, 'c>(&self, checker: &'c mut Checker<'a, 'state>, typ: TypeHandle) {
        let display = trace_display(checker, typ);
        if let Some(typ) = wrap_type(Some(typ), Some(checker), display) {
            self.recorder.borrow_mut().record_type(typ);
        }
    }

    pub fn push(
        &self,
        phase: tracing::Phase,
        name: &str,
        mut args: HashMap<String, tracing::Any>,
        separate_begin_and_end: bool,
    ) -> Box<dyn FnOnce() + 'a> {
        if !separate_begin_and_end {
            let pop = self.tracing.borrow_mut().push(
                phase,
                name,
                self.copy_with_checker_index(args),
                separate_begin_and_end,
            );
            let tracing = self.tracing.clone();
            return Box::new(move || {
                let mut tracing = tracing.borrow_mut();
                pop(&mut **tracing);
            });
        }

        // PORT NOTE: reshaped for borrowck. This preserves the Go push/pop
        // mutation order without keeping a restore closure borrowing `args`.
        let previous_checker_index =
            args.insert("checkerId".to_string(), self.checker_index.into());
        let args_with_checker = args.clone();
        let pop = self.tracing.borrow_mut().push(
            phase,
            name,
            args_with_checker.clone(),
            separate_begin_and_end,
        );
        if let Some(previous) = previous_checker_index {
            args.insert("checkerId".to_string(), previous);
        } else {
            args.remove("checkerId");
        }

        let checker_index = self.checker_index;
        let tracing = self.tracing.clone();
        Box::new(move || {
            // PORT NOTE: reshaped for borrowck. Match the Go closure by adding
            // checkerId for the end event, invoking pop, then restoring args.
            let previous_checker_index = args.insert("checkerId".to_string(), checker_index.into());
            let mut tracing = tracing.borrow_mut();
            pop(&mut **tracing);
            if let Some(previous) = previous_checker_index {
                args.insert("checkerId".to_string(), previous);
            } else {
                args.remove("checkerId");
            }
        })
    }

    pub fn instant(&self, phase: tracing::Phase, name: &str, args: HashMap<String, tracing::Any>) {
        self.tracing
            .borrow_mut()
            .instant(phase, name, self.copy_with_checker_index(args));
    }

    fn copy_with_checker_index(
        &self,
        args: HashMap<String, tracing::Any>,
    ) -> HashMap<String, tracing::Any> {
        let mut with_checker_index = HashMap::with_capacity(args.len() + 1);
        with_checker_index.extend(args);
        with_checker_index.insert("checkerId".to_string(), self.checker_index.into());
        with_checker_index
    }

    fn temporarily_add_checker_index<'b>(
        &self,
        args: &'b mut HashMap<String, tracing::Any>,
    ) -> (
        HashMap<String, tracing::Any>,
        Box<dyn FnOnce(&mut HashMap<String, tracing::Any>) + 'b>,
    ) {
        temporarily_add_checker_index_for_index(args, self.checker_index)
    }
}

fn temporarily_add_checker_index_for_index<'a>(
    args: &'a mut HashMap<String, tracing::Any>,
    checker_index: i32,
) -> (
    HashMap<String, tracing::Any>,
    Box<dyn FnOnce(&mut HashMap<String, tracing::Any>) + 'a>,
) {
    let previous = args.insert("checkerId".to_string(), checker_index.into());

    let current = args.clone();
    (
        current,
        Box::new(move |args: &mut HashMap<String, tracing::Any>| {
            if let Some(previous) = previous {
                args.insert("checkerId".to_string(), previous);
            } else {
                args.remove("checkerId");
            }
        }),
    )
}

// tracedTypeAdapter adapts a Type to the tracing.TracedType interface
struct TracedTypeAdapter<'a, 'state, 'c> {
    t: TypeHandle,
    checker: Option<&'c Checker<'a, 'state>>,
    display: String,
}

impl<'a, 'state, 'c> TracedTypeAdapter<'a, 'state, 'c> {
    fn wrap_type(&self, t: Option<TypeHandle>) -> Option<Box<dyn tracing::TracedType + '_>> {
        wrap_type(t, self.checker, String::new())
    }

    fn wrap_types(&self, types: Vec<TypeHandle>) -> Vec<Box<dyn tracing::TracedType + '_>> {
        if types.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::with_capacity(types.len());
        for t in types {
            result.push(self.wrap_type(Some(t)).unwrap());
        }
        result
    }
}

impl<'a, 'state, 'c> tracing::TracedType for TracedTypeAdapter<'a, 'state, 'c> {
    fn id(&self) -> u32 {
        let Some(checker) = self.checker.as_ref() else {
            return 0;
        };
        checker.type_id(self.t) as u32
    }

    fn format_flags(&self) -> Vec<String> {
        let Some(checker) = self.checker.as_ref() else {
            return Vec::new();
        };
        format_type_flags(checker.type_flags(self.t))
    }

    fn is_conditional(&self) -> bool {
        let Some(checker) = self.checker.as_ref() else {
            return false;
        };
        checker.type_flags(self.t) & TYPE_FLAGS_CONDITIONAL != 0
    }

    fn symbol(&self) -> Option<ast::SymbolIdentity> {
        let checker = self.checker.as_ref()?;
        checker
            .type_symbol_identity(self.t)
            .map(SymbolIdentity::ast_identity)
    }

    fn alias_symbol(&self) -> Option<ast::SymbolIdentity> {
        let checker = self.checker.as_ref()?;
        let alias = checker.type_alias(self.t)?;
        checker
            .type_alias_symbol_identity(alias)
            .map(SymbolIdentity::ast_identity)
    }

    fn symbol_name(&self, symbol: ast::SymbolIdentity) -> Option<String> {
        let checker = self.checker.as_ref()?;
        Some(
            checker
                .symbol_identity_name(SymbolIdentity::from_symbol_handle(symbol.symbol_handle()))
                .to_string(),
        )
    }

    fn first_symbol_declaration(&self, symbol: ast::SymbolIdentity) -> Option<ast::Node> {
        let checker = self.checker.as_ref()?;
        checker.first_symbol_identity_declaration(SymbolIdentity::from_symbol_handle(
            symbol.symbol_handle(),
        ))
    }

    fn alias_type_arguments(&self) -> Vec<Box<dyn tracing::TracedType + '_>> {
        let Some(checker) = self.checker.as_ref() else {
            return Vec::new();
        };
        let type_arguments = checker
            .type_alias_record(self.t)
            .map(|alias| alias.type_arguments.clone())
            .unwrap_or_default();
        self.wrap_types(type_arguments)
    }

    fn intrinsic_name(&self) -> String {
        let Some(checker) = self.checker.as_ref() else {
            return String::new();
        };
        if checker.type_flags(self.t) & TYPE_FLAGS_INTRINSIC == 0 {
            return String::new();
        }
        checker
            .type_record(self.t)
            .as_intrinsic_type()
            .intrinsic_name
            .clone()
    }

    fn union_types(&self) -> Vec<Box<dyn tracing::TracedType + '_>> {
        let Some(checker) = self.checker.as_ref() else {
            return Vec::new();
        };
        if checker.type_flags(self.t) & TYPE_FLAGS_UNION == 0 {
            return Vec::new();
        }
        self.wrap_types(
            checker
                .type_record(self.t)
                .as_union_type()
                .union_or_intersection
                .types
                .clone(),
        )
    }

    fn intersection_types(&self) -> Vec<Box<dyn tracing::TracedType + '_>> {
        let Some(checker) = self.checker.as_ref() else {
            return Vec::new();
        };
        if checker.type_flags(self.t) & TYPE_FLAGS_INTERSECTION == 0 {
            return Vec::new();
        }
        self.wrap_types(
            checker
                .type_record(self.t)
                .as_intersection_type()
                .union_or_intersection
                .types
                .clone(),
        )
    }

    fn index_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_INDEX == 0 {
            return None;
        }
        self.wrap_type(checker.type_record(self.t).as_index_type().target)
    }

    fn indexed_access_object_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_INDEXED_ACCESS == 0 {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_indexed_access_type()
                .object_type,
        )
    }

    fn indexed_access_index_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_INDEXED_ACCESS == 0 {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_indexed_access_type()
                .index_type,
        )
    }

    fn conditional_check_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_CONDITIONAL == 0 {
            return None;
        }
        self.wrap_type(checker.type_record(self.t).as_conditional_type().check_type)
    }

    fn conditional_extends_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_CONDITIONAL == 0 {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_conditional_type()
                .extends_type,
        )
    }

    fn conditional_true_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_CONDITIONAL == 0 {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_conditional_type()
                .resolved_true_type,
        )
    }

    fn conditional_false_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_CONDITIONAL == 0 {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_conditional_type()
                .resolved_false_type,
        )
    }

    fn substitution_base_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_SUBSTITUTION == 0 {
            return None;
        }
        self.wrap_type(checker.type_record(self.t).as_substitution_type().base_type)
    }

    fn substitution_constraint_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_SUBSTITUTION == 0 {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_substitution_type()
                .constraint,
        )
    }

    fn reference_target(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_OBJECT == 0
            || checker.object_flags(self.t) & OBJECT_FLAGS_REFERENCE == 0
        {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_type_reference()
                .unwrap()
                .object
                .target,
        )
    }

    fn reference_type_arguments(&self) -> Vec<Box<dyn tracing::TracedType + '_>> {
        let Some(checker) = self.checker.as_ref() else {
            return Vec::new();
        };
        if checker.type_flags(self.t) & TYPE_FLAGS_OBJECT == 0
            || checker.object_flags(self.t) & OBJECT_FLAGS_REFERENCE == 0
        {
            return Vec::new();
        }
        self.wrap_types(
            checker
                .type_record(self.t)
                .as_type_reference()
                .unwrap()
                .resolved_type_arguments
                .clone()
                .unwrap_or_default(),
        )
    }

    fn reference_node(&self) -> Option<ast::Node> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_OBJECT == 0
            || checker.object_flags(self.t) & OBJECT_FLAGS_REFERENCE == 0
        {
            return None;
        }
        checker
            .type_record(self.t)
            .as_type_reference()
            .unwrap()
            .node
    }

    fn reverse_mapped_source_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_OBJECT == 0
            || checker.object_flags(self.t) & OBJECT_FLAGS_REVERSE_MAPPED == 0
        {
            return None;
        }
        self.wrap_type(checker.type_record(self.t).as_reverse_mapped_type().source)
    }

    fn reverse_mapped_mapped_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_OBJECT == 0
            || checker.object_flags(self.t) & OBJECT_FLAGS_REVERSE_MAPPED == 0
        {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_reverse_mapped_type()
                .mapped_type,
        )
    }

    fn reverse_mapped_constraint_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_OBJECT == 0
            || checker.object_flags(self.t) & OBJECT_FLAGS_REVERSE_MAPPED == 0
        {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_reverse_mapped_type()
                .constraint_type,
        )
    }

    fn evolving_array_element_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_OBJECT == 0
            || checker.object_flags(self.t) & OBJECT_FLAGS_EVOLVING_ARRAY == 0
        {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_evolving_array_type()
                .element_type,
        )
    }

    fn evolving_array_final_type(&self) -> Option<Box<dyn tracing::TracedType + '_>> {
        let checker = self.checker.as_ref()?;
        if checker.type_flags(self.t) & TYPE_FLAGS_OBJECT == 0
            || checker.object_flags(self.t) & OBJECT_FLAGS_EVOLVING_ARRAY == 0
        {
            return None;
        }
        self.wrap_type(
            checker
                .type_record(self.t)
                .as_evolving_array_type()
                .final_array_type,
        )
    }

    fn is_tuple(&self) -> bool {
        let Some(checker) = self.checker.as_ref() else {
            return false;
        };
        checker.is_tuple_type(self.t)
    }

    fn pattern(&self) -> Option<ast::Node> {
        let checker = self.checker.as_ref()?;
        checker.semantic_state.pattern_for_type(self.t)
    }

    fn get_location(&self, node: ast::Node) -> Option<tracing::Location> {
        let checker = self.checker.as_ref()?;
        tracing::get_location(node, checker.try_source_file_for_node(node)?)
    }

    fn recursion_identity(&self) -> Option<usize> {
        use std::hash::{Hash, Hasher};

        let checker = self.checker.as_ref()?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        get_recursion_identity(checker, self.t).hash(&mut hasher);
        Some(hasher.finish() as usize)
    }

    fn display(&self) -> String {
        self.display.clone()
    }
}

fn wrap_type<'a, 'state, 'c>(
    t: Option<TypeHandle>,
    checker: Option<&'c Checker<'a, 'state>>,
    display: String,
) -> Option<Box<dyn tracing::TracedType + 'c>>
where
    'a: 'c,
    'state: 'c,
{
    let t = t?;
    Some(Box::new(TracedTypeAdapter {
        t,
        checker,
        display,
    }))
}

fn trace_display(checker: &mut Checker<'_, '_>, t: TypeHandle) -> String {
    let flags = checker.type_flags(t);
    let object_flags = checker.object_flags(t);
    if object_flags & OBJECT_FLAGS_ANONYMOUS == 0
        && flags
            & (TYPE_FLAGS_LITERAL
                | TYPE_FLAGS_TEMPLATE_LITERAL
                | TYPE_FLAGS_UNION
                | TYPE_FLAGS_INTERSECTION)
            == 0
    {
        return String::new();
    }
    catch_unwind(AssertUnwindSafe(|| checker.type_to_string(t, None))).unwrap_or_default()
}
