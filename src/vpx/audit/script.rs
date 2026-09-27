//! The checks that parse the script, behind the `script-audit` feature.

use super::{Kind, NameKind, ScriptLocation, VPX};
use crate::vpx::gameitem::GameItemEnum;
use std::collections::HashSet;
use vbscript::lexer::LineIndex;
use vbscript::parser::Parser;
use vbscript::parser::ast::{
    Expr, ExprKind, Item, ItemKind, MemberAccess, Name, PropertyType, SetRhs, Spanned, Stmt,
    StmtKind,
};
use vbscript::parser::visit::{
    Visitor, walk_expr, walk_item, walk_items, walk_member_access, walk_stmt,
};

/// A name the script declares, as it spells it, and where it does
#[derive(Clone)]
struct Declared {
    name: String,
    location: ScriptLocation,
}

/// What one pass over the script collected
struct Scan<'script> {
    /// to find the line and column of a name
    index: LineIndex<'script>,
    option_explicit: bool,
    /// every identifier seen, lowercased, like vpinball's audit bag
    identifiers: HashSet<String>,
    /// declared sub/function names, qualified by class so that a method
    /// name reused across classes is not a duplicate
    declared: Vec<Declared>,
    /// the class currently being scanned, if any
    current_class: Option<String>,
    /// names declared at script level: variables, constants, subs,
    /// functions and classes, which all live in the namespace the
    /// table items are in
    script_level: Vec<Declared>,
    /// how many procedures deep the scan is
    depth: usize,
    /// subs and functions declared at script level, the only ones
    /// vpinball can dispatch events to
    procedures: Vec<Declared>,
    /// items handed to core.vbs `vpmBuildEvent` or `InitTimer`, which
    /// build the timer handler at runtime
    built_events: HashSet<String>,
    /// variables declared at script level, in declaration order
    variables: Vec<Declared>,
    /// the sub or function being scanned, with what it declares and
    /// names
    procedure: Option<Procedure>,
    /// `procedure.variable` for every local a procedure declares and
    /// never names
    unused_locals: Vec<Declared>,
    /// where the script assigns a member named `ID`, with the variable
    /// it assigns it on when that is a plain one
    id_assignments: Vec<(Option<String>, ScriptLocation)>,
    /// classes that declare an `ID` the script can assign
    id_classes: HashSet<String>,
    /// variables the script sets to a new instance, with the class
    instances: Vec<(String, String)>,
    /// the variable of each enclosing `With`, when it is a plain one
    with_objects: Vec<Option<String>>,
}

struct Procedure {
    name: String,
    dims: Vec<Declared>,
    identifiers: HashSet<String>,
}

/// Table script globals the standard scripts read: core.vbs looks most
/// of them up with `Eval`, controller.vbs, B2B.vbs and the machine
/// scripts name the rest. A table declares them for those scripts, so
/// never naming them again is not dead code
const STANDARD_SCRIPT_GLOBALS: [&str; 24] = [
    "b2bon",
    "b2scgamename",
    "b2son",
    "ballmass",
    "ballsize",
    "cgamename",
    "csinglelflip",
    "csinglerflip",
    "gameonsolenoid",
    "noupperrightflipper",
    "nvoffset",
    "scoin",
    "sflipperoff",
    "sflipperon",
    "ssolenoidoff",
    "ssolenoidon",
    "uselamps",
    "usepdbleds",
    "usesolenoids",
    "usevpmcoloreddmd",
    "usevpmdmd",
    "usevpmmodsol",
    "usevpmnvram",
    "vpmballimage",
];

/// The events vpinball fires on script objects, from vpinball.idl
const EVENTS: [&str; 21] = [
    "init",
    "timer",
    "hit",
    "unhit",
    "animate",
    "limiteos",
    "limitbos",
    "spin",
    "slingshot",
    "raised",
    "dropped",
    "paused",
    "unpaused",
    "sounddone",
    "playdone",
    "optionevent",
    "musicdone",
    "keyup",
    "keydown",
    "exit",
    "collide",
];

pub(super) fn check(vpx: &VPX, findings: &mut Vec<Kind>) {
    let script = &vpx.gamedata.code.string;
    if script.trim().is_empty() {
        return;
    }
    let items = match Parser::new(script).file() {
        Ok(items) => items,
        Err(e) => {
            // the parser counts from 1, 0 stands for unknown
            let location = (e.line() > 0).then(|| ScriptLocation {
                line: e.line(),
                column: (e.column() > 0).then_some(e.column()),
            });
            findings.push(Kind::ScriptParseError {
                detail: e.message().to_string(),
                location,
            });
            return;
        }
    };

    let mut scan = Scan::new(script);
    walk_items(&mut scan, &items);

    if !scan.option_explicit {
        findings.push(Kind::MissingOptionExplicit);
    }

    let mut seen: HashSet<String> = HashSet::new();
    for procedure in &scan.declared {
        if !seen.insert(procedure.name.to_lowercase()) {
            findings.push(Kind::DuplicateProcedure {
                name: procedure.name.clone(),
                location: procedure.location,
            });
        }
    }

    // only bare Execute, like vpinball: it evaluates runtime-built code
    // and can stutter in game logic. ExecuteGlobal is normal at load time
    // (tables inject their controller and backglass scripts with it), so
    // flagging it would be noise
    if scan.identifiers.contains("execute") {
        findings.push(Kind::ExecuteUsed);
    }

    let timers: HashSet<String> = vpx
        .gameitems
        .iter()
        .filter_map(|item| match item {
            GameItemEnum::Timer(timer) => Some(timer.name.to_lowercase()),
            _ => None,
        })
        .collect();

    let uses_vpm = scan.identifiers.contains("loadvpm") || scan.identifiers.contains("loadvpmalt");
    if uses_vpm {
        if !timers.contains("pinmametimer") {
            findings.push(Kind::MissingPinMameTimer);
        }
        if !scan.identifiers.contains("vpminit") {
            findings.push(Kind::MissingVpmInit);
        }
    }
    if scan.identifiers.contains("vpmtimer") && !timers.contains("pulsetimer") {
        findings.push(Kind::MissingPulseTimer);
    }

    // a script level name equal to an item or collection name hides it
    let items: HashSet<String> = vpx
        .gameitems
        .iter()
        .map(|item| item.name().to_lowercase())
        .collect();
    let collections: HashSet<String> = vpx
        .collections
        .iter()
        .map(|collection| collection.name.to_lowercase())
        .collect();
    let mut reported: HashSet<String> = HashSet::new();
    for declared in &scan.script_level {
        let lower = declared.name.to_lowercase();
        let kind = if items.contains(&lower) {
            NameKind::GameItem
        } else if collections.contains(&lower) {
            NameKind::Collection
        } else {
            continue;
        };
        if reported.insert(lower) {
            findings.push(Kind::ScriptNameShadowsItem {
                name: declared.name.clone(),
                kind,
                location: declared.location,
            });
        }
    }

    if scan.identifiers.contains("rnd") && !scan.identifiers.contains("randomize") {
        findings.push(Kind::RndWithoutRandomize);
    }

    // declared and never named again, not even inside a string handed
    // to Eval or Execute: dead declarations
    let literal_words: HashSet<String> = super::assets::script_literals(&vpx.gamedata.code.string)
        .iter()
        .flat_map(|literal| {
            literal
                .text
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .filter(|word| !word.is_empty())
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
        })
        .collect();
    let mut seen: HashSet<String> = HashSet::new();
    for variable in &scan.variables {
        let lower = variable.name.to_lowercase();
        if seen.insert(lower.clone())
            && !scan.identifiers.contains(&lower)
            && !literal_words.contains(&lower)
            && !STANDARD_SCRIPT_GLOBALS.contains(&lower.as_str())
        {
            findings.push(Kind::UnusedVariable {
                name: variable.name.clone(),
                location: variable.location,
            });
        }
    }
    for local in &scan.unused_locals {
        let variable = local.name.rsplit('.').next().unwrap_or(&local.name);
        if !literal_words.contains(&variable.to_lowercase()) {
            findings.push(Kind::UnusedLocalVariable {
                name: local.name.clone(),
                location: local.location,
            });
        }
    }

    // enabled timers nothing handles, and handlers nothing fires
    let procedures: HashSet<String> = scan
        .procedures
        .iter()
        .map(|procedure| procedure.name.to_lowercase())
        .collect();
    let handled_by_collection: HashSet<String> = vpx
        .collections
        .iter()
        .filter(|collection| {
            collection.fire_events
                && procedures.contains(&format!("{}_timer", collection.name.to_lowercase()))
        })
        .flat_map(|collection| collection.items.iter().map(|item| item.to_lowercase()))
        .collect();
    for item in &vpx.gameitems {
        let Some(timer) = item.timer() else {
            continue;
        };
        let name = item.name().to_lowercase();
        let handler = format!("{name}_timer");
        if !timer.is_enabled
            || name.is_empty()
            || procedures.contains(&handler)
            || scan.identifiers.contains(&handler)
            || handled_by_collection.contains(&name)
            || scan.built_events.contains(&name)
            || matches!(name.as_str(), "pinmametimer" | "pulsetimer")
        {
            continue;
        }
        findings.push(Kind::TimerWithoutHandler {
            item: item.name().to_string(),
            interval: timer.interval,
        });
    }
    let table = vpx.gamedata.name.to_lowercase();
    for handler in &scan.procedures {
        let lower = handler.name.to_lowercase();
        let orphan = lower.rsplit_once('_').is_some_and(|(object, event)| {
            !object.is_empty()
                && EVENTS.contains(&event)
                && object != table
                && !items.contains(object)
                && !collections.contains(object)
                && !scan.identifiers.contains(&lower)
        });
        if orphan {
            findings.push(Kind::HandlerWithoutItem {
                name: handler.name.clone(),
                location: handler.location,
            });
        }
    }

    // like vpinball: any mention of a static primitive's name, reading a
    // property is fine but writing one has no effect once it is baked
    let script_toggles_prerendering = scan.identifiers.contains("disablestaticprerendering");
    for item in &vpx.gameitems {
        if let GameItemEnum::Primitive(primitive) = item
            && primitive.static_rendering
            && scan.identifiers.contains(&primitive.name.to_lowercase())
        {
            findings.push(Kind::StaticPrimitiveInScript {
                name: primitive.name.clone(),
                item: super::references::item_label(item),
                script_toggles_prerendering,
            });
        }
    }

    // the ball is the only vpinball object with an ID property, so
    // anything but an instance of a script class with one is a ball
    let own_ids: HashSet<&str> = scan
        .instances
        .iter()
        .filter(|(_, class)| scan.id_classes.contains(class))
        .map(|(variable, _)| variable.as_str())
        .collect();
    for (variable, location) in &scan.id_assignments {
        if !variable
            .as_deref()
            .is_some_and(|variable| variable == "me" || own_ids.contains(variable))
        {
            findings.push(Kind::BallIdAssigned {
                location: *location,
            });
        }
    }
}

impl<'ast> Visitor<'ast> for Scan<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        match &item.node {
            ItemKind::OptionExplicit => self.option_explicit = true,
            ItemKind::Class {
                name,
                members,
                dims,
                member_accessors,
                ..
            } => {
                let is_id = |name: &Name| name.eq_ignore_ascii_case("id");
                let declares_id = members
                    .iter()
                    .flat_map(|member| &member.properties)
                    .chain(dims.iter().flatten())
                    .any(|var| is_id(&var.name))
                    || member_accessors.iter().any(|accessor| {
                        accessor.node.property_type != PropertyType::Get
                            && is_id(&accessor.node.name)
                    });
                if declares_id {
                    self.id_classes.insert(name.to_lowercase());
                }
                let class = self.declare(name);
                self.script_level.push(class);
                self.current_class = Some(name.to_string());
                walk_item(self, item);
                self.current_class = None;
                return;
            }
            ItemKind::Statement(_) => {}
            ItemKind::Const { values, .. } => {
                let constants = self.declare_all(values.iter().map(|(name, _)| name));
                self.script_level.extend(constants);
            }
            ItemKind::Variable { vars, .. } => {
                let variables = self.declare_all(vars.iter().map(|var| &var.name));
                self.script_level.extend(variables.iter().cloned());
                self.variables.extend(variables);
            }
        }
        walk_item(self, item);
    }

    /// A `Property Get`, `Let` or `Set`. They share a name, so they do
    /// not count as declared more than once
    fn visit_member_access(&mut self, member_access: &'ast Spanned<MemberAccess>) {
        let qualified = self.qualified(&member_access.name);
        self.procedure(qualified, |scan| walk_member_access(scan, member_access));
    }

    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match &stmt.node {
            StmtKind::Sub { name, .. } | StmtKind::Function { name, .. } => {
                let qualified = self.qualified(name);
                let procedure = self.declare(name);
                if self.current_class.is_none() {
                    self.script_level.push(procedure.clone());
                    if self.depth == 0 {
                        self.procedures.push(procedure.clone());
                    }
                }
                self.declared.push(Declared {
                    name: qualified.clone(),
                    location: procedure.location,
                });
                self.procedure(qualified, |scan| walk_stmt(scan, stmt));
                return;
            }
            // a Dim inside a procedure is local, but VBScript hoists
            // nothing: only script level declarations shadow items
            StmtKind::Dim { vars } if self.current_class.is_none() && self.depth == 0 => {
                let variables = self.declare_all(vars.iter().map(|var| &var.name));
                self.script_level.extend(variables.iter().cloned());
                self.variables.extend(variables);
                return;
            }
            StmtKind::Dim { vars } => {
                let locals = self.declare_all(vars.iter().map(|var| &var.name));
                if let Some(procedure) = &mut self.procedure {
                    procedure.dims.extend(locals);
                }
                return;
            }
            StmtKind::ReDim { vars, .. } => {
                for var in vars {
                    self.ident(&var.name);
                }
            }
            StmtKind::Const(values) if self.current_class.is_none() && self.depth == 0 => {
                let constants = self.declare_all(values.iter().map(|(name, _)| name));
                self.script_level.extend(constants);
            }
            StmtKind::SubCall { fn_name, args } => self.built_event(&fn_name.0, args),
            StmtKind::Call(fi) => {
                if let ExprKind::FnApplication { callee, args } = &fi.0.node {
                    self.built_event(callee, args);
                }
            }
            StmtKind::Assignment { full_ident, .. } => {
                if let ExprKind::MemberExpression { base, property } = &full_ident.0.node
                    && property.eq_ignore_ascii_case("id")
                {
                    let variable = self.variable(base);
                    let location = self.declare(property).location;
                    self.id_assignments.push((variable, location));
                }
            }
            StmtKind::Set {
                var,
                rhs: SetRhs::Expr(value),
            } => {
                if let ExprKind::New(class) = &value.node
                    && let Some(variable) = self.variable(&var.0)
                {
                    self.instances.push((variable, class.to_lowercase()));
                }
            }
            StmtKind::With { object, .. } => {
                let variable = self.variable(&object.0);
                self.with_objects.push(variable);
                walk_stmt(self, stmt);
                self.with_objects.pop();
                return;
            }
            StmtKind::ForStmt { counter, .. } => self.ident(counter),
            StmtKind::ForEachStmt { element, .. } => self.ident(element),
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        match &expr.node {
            ExprKind::Ident(name) => self.ident(name),
            ExprKind::New(name) | ExprKind::MemberExpression { property: name, .. } => {
                self.identifiers.insert(name.to_lowercase());
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

impl<'script> Scan<'script> {
    fn new(script: &'script str) -> Self {
        Scan {
            index: LineIndex::new(script),
            option_explicit: false,
            identifiers: HashSet::new(),
            declared: Vec::new(),
            current_class: None,
            script_level: Vec::new(),
            depth: 0,
            procedures: Vec::new(),
            built_events: HashSet::new(),
            variables: Vec::new(),
            procedure: None,
            unused_locals: Vec::new(),
            id_assignments: Vec::new(),
            id_classes: HashSet::new(),
            instances: Vec::new(),
            with_objects: Vec::new(),
        }
    }

    /// A name with the line and column the script declares it at
    fn declare(&self, name: &Name) -> Declared {
        let (line, column) = self.index.line_column(name.span.start);
        Declared {
            name: name.to_string(),
            location: ScriptLocation {
                line,
                column: Some(column),
            },
        }
    }

    fn declare_all<'a>(&self, names: impl Iterator<Item = &'a Name>) -> Vec<Declared> {
        names.map(|name| self.declare(name)).collect()
    }

    /// The name of a procedure, with the class it is in
    fn qualified(&self, name: &str) -> String {
        match &self.current_class {
            Some(class) => format!("{class}.{name}"),
            None => name.to_string(),
        }
    }

    /// Scans the body of a sub, function or property with `walk` and
    /// reports the locals it declares but never names
    fn procedure(&mut self, name: String, walk: impl FnOnce(&mut Self)) {
        let outer = self.procedure.replace(Procedure {
            name,
            dims: Vec::new(),
            identifiers: HashSet::new(),
        });
        self.depth += 1;
        walk(self);
        self.depth -= 1;
        if let Some(procedure) = self.procedure.take() {
            for dim in &procedure.dims {
                if !procedure.identifiers.contains(&dim.name.to_lowercase()) {
                    self.unused_locals.push(Declared {
                        name: format!("{}.{}", procedure.name, dim.name),
                        location: dim.location,
                    });
                }
            }
        }
        self.procedure = outer;
    }

    /// `vpmBuildEvent item, ...` and `vpmTimer.InitTimer item, ...` give
    /// the item a handler at runtime
    fn built_event(&mut self, callee: &Expr, args: &[Option<Expr>]) {
        let name = match &callee.node {
            ExprKind::Ident(name) | ExprKind::MemberExpression { property: name, .. } => name,
            _ => return,
        };
        if !name.eq_ignore_ascii_case("vpmbuildevent") && !name.eq_ignore_ascii_case("inittimer") {
            return;
        }
        if let Some(Some(item)) = args.first() {
            // `InitTimer (item), True` hands the item over by value
            let mut item = item;
            while let ExprKind::Paren(inner) = &item.node {
                item = inner;
            }
            if let ExprKind::Ident(item) = &item.node {
                self.built_events.insert(item.to_lowercase());
            }
        }
    }

    /// The variable, lower cased, an expression is or indexes, as in
    /// `ball` or `balls(i)`; the one of the enclosing `With` for `.`
    fn variable(&self, expr: &Expr) -> Option<String> {
        match &expr.node {
            ExprKind::Ident(name) => Some(name.to_lowercase()),
            ExprKind::Paren(inner) => self.variable(inner),
            ExprKind::FnApplication { callee, .. } if matches!(callee.node, ExprKind::Ident(_)) => {
                self.variable(callee)
            }
            ExprKind::WithScoped => self.with_objects.last().cloned().flatten(),
            _ => None,
        }
    }

    /// A name the script uses, for the script and for the procedure
    /// being scanned
    fn ident(&mut self, name: &str) {
        let lower = name.to_lowercase();
        if let Some(procedure) = &mut self.procedure {
            procedure.identifiers.insert(lower.clone());
        }
        self.identifiers.insert(lower);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::audit::test_support::*;
    use crate::vpx::audit::{Kind, NameKind, Severity, audit_kinds};
    use pretty_assertions::assert_eq;

    /// like clean_vpx but with a known good, CRLF, Option Explicit script
    fn scripted(body: &str) -> VPX {
        let mut vpx = clean_vpx();
        let script = format!("Option Explicit\r\n{}", body.replace('\n', "\r\n"));
        vpx.gamedata.code.string = script;
        vpx
    }

    fn script_findings(vpx: &VPX) -> Vec<Kind> {
        audit_kinds(vpx)
            .into_iter()
            .filter(|f| {
                matches!(
                    f,
                    Kind::ScriptParseError { .. }
                        | Kind::MissingOptionExplicit
                        | Kind::DuplicateProcedure { .. }
                        | Kind::ExecuteUsed
                        | Kind::MissingPinMameTimer
                        | Kind::MissingVpmInit
                        | Kind::MissingPulseTimer
                        | Kind::ScriptNameShadowsItem { .. }
                        | Kind::RndWithoutRandomize
                        | Kind::TimerWithoutHandler { .. }
                        | Kind::HandlerWithoutItem { .. }
                        | Kind::StaticPrimitiveInScript { .. }
                        | Kind::UnusedVariable { .. }
                        | Kind::UnusedLocalVariable { .. }
                        | Kind::BallIdAssigned { .. }
                )
            })
            .collect()
    }

    /// A location in the script. Line 1 is the `Option Explicit` that
    /// `scripted` puts before the body
    fn at(line: usize, column: usize) -> ScriptLocation {
        ScriptLocation {
            line,
            column: Some(column),
        }
    }

    fn unused(name: &str, location: ScriptLocation) -> Kind {
        Kind::UnusedVariable {
            name: name.to_string(),
            location,
        }
    }

    fn unused_local(name: &str, location: ScriptLocation) -> Kind {
        Kind::UnusedLocalVariable {
            name: name.to_string(),
            location,
        }
    }

    #[test]
    fn variables_declared_but_never_named_are_reported() {
        let vpx = scripted(
            "Dim used, dead, viaEval\nPrivate alsoDead\nDim BallSize\n\
             Sub Table1_Init\n    used = Eval(\"viaEval\")\nEnd Sub\n",
        );
        let findings = script_findings(&vpx);
        // each at the name, not at its `Dim` or `Private`
        assert_eq!(
            findings,
            vec![unused("dead", at(2, 11)), unused("alsoDead", at(3, 9))]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
        assert_eq!(
            findings[0].to_string(),
            "script declares the variable \"dead\" and never uses it"
        );
    }

    #[test]
    fn a_property_of_a_class_is_scanned_like_a_procedure() {
        let vpx = scripted(
            "Dim counter, dead\n\
             Class Foo\n\
             \x20   Public Property Get Count()\n\
             \x20       Dim unusedLocal, usedLocal\n\
             \x20       usedLocal = counter\n\
             \x20       Count = usedLocal\n\
             \x20   End Property\n\
             End Class\n",
        );
        assert_eq!(
            script_findings(&vpx),
            vec![
                // `counter` is only named in the property
                unused("dead", at(2, 14)),
                unused_local("Foo.Count.unusedLocal", at(5, 13)),
            ]
        );
    }

    #[test]
    fn locals_a_procedure_never_names_are_reported() {
        let vpx = scripted(
            "Sub Table1_Init\n    Dim x, y\n    x = 1\nEnd Sub\n\
             Function Twice(n)\n    Dim tmp\n    Twice = n * 2\nEnd Function\n",
        );
        let findings = script_findings(&vpx);
        assert_eq!(
            findings,
            vec![
                unused_local("Table1_Init.y", at(3, 12)),
                unused_local("Twice.tmp", at(7, 9)),
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
        assert_eq!(
            findings[0].to_string(),
            "script declares the local variable \"Table1_Init.y\" and its procedure never uses it"
        );
    }

    #[test]
    fn loop_counters_conditions_case_tests_and_redim_count_as_use() {
        let vpx = scripted(
            "Dim i, e, n, c, s, arr()\n\
             Sub Table1_Init\n\
             \x20   For i = 1 To 3\n    Next\n\
             \x20   For Each e In arr\n    Next\n\
             \x20   Do While n > 0\n    Loop\n\
             \x20   Select Case c\n        Case s\n    End Select\n\
             \x20   ReDim arr(2)\n\
             End Sub\n",
        );
        assert_eq!(script_findings(&vpx), vec![]);
    }

    #[test]
    fn a_timer_handed_over_in_parentheses_has_a_handler() {
        use crate::vpx::gameitem::timer::Timer;
        // parentheses around an argument pass it by value, it is the same item
        let mut vpx = scripted(
            "Sub Table1_Init\n    vpmTimer.InitTimer (Pulsed), True\n    vpmBuildEvent(Built)\nEnd Sub\n",
        );
        // vpmTimer needs the PulseTimer, which core.vbs handles
        for (name, interval) in [("Pulsed", 100), ("Built", 100), ("PulseTimer", 1)] {
            let mut timer = Timer {
                name: name.to_string(),
                ..Timer::default()
            };
            timer.timer.is_enabled = true;
            timer.timer.interval = interval;
            vpx.gameitems.push(GameItemEnum::Timer(timer));
        }
        vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
        assert_eq!(script_findings(&vpx), vec![]);
    }

    #[test]
    fn an_enabled_timer_without_a_handler_is_reported() {
        use crate::vpx::collection::Collection;
        use crate::vpx::gameitem::timer::Timer;
        let mut vpx = scripted(
            "Sub Handled_Timer\nEnd Sub\n\
             Sub Group_Timer(idx)\nEnd Sub\n\
             Sub Table1_Init\n    vpmTimer.InitTimer Pulsed, True\n    Call vpmBuildEvent(Built, \"Timer\", \"x\")\nEnd Sub\n",
        );
        for (name, enabled, interval) in [
            ("Handled", true, 100),
            ("Grouped", true, 100),
            ("Pulsed", true, 100),
            ("Built", true, 100),
            ("PulseTimer", true, 1),
            ("Off", false, 100),
            ("Orphan", true, -1),
            ("Lonely", true, 250),
        ] {
            let mut timer = Timer {
                name: name.to_string(),
                ..Timer::default()
            };
            timer.timer.is_enabled = enabled;
            timer.timer.interval = interval;
            vpx.gameitems.push(GameItemEnum::Timer(timer));
        }
        vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
        vpx.collections.push(Collection {
            name: "Group".to_string(),
            items: vec!["Grouped".to_string()],
            fire_events: true,
            stop_single_events: false,
            group_elements: false,
        });
        vpx.gamedata.collections_size = 1;
        let findings = script_findings(&vpx);
        assert_eq!(
            findings,
            vec![
                Kind::TimerWithoutHandler {
                    item: "Orphan".to_string(),
                    interval: -1,
                },
                Kind::TimerWithoutHandler {
                    item: "Lonely".to_string(),
                    interval: 250,
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
        assert_eq!(
            findings[0].to_string(),
            "timer of \"Orphan\" fires every frame but the script has no Orphan_Timer handler"
        );
    }

    #[test]
    fn handlers_for_missing_items_are_reported() {
        use crate::vpx::gameitem::wall::Wall;
        let mut vpx = scripted(
            "Sub Bumper1_Hit\nEnd Sub\n\
             Sub Arch1_Hit\nEnd Sub\n\
             Sub TBWR_Timer\nEnd Sub\n\
             Sub Game_Init\nEnd Sub\n\
             Sub Table1_KeyDown(ByVal key)\nEnd Sub\n\
             Sub Helper_Timer\nEnd Sub\n\
             Sub Table1_Init\n    Game_Init\n    Helper_Timer\nEnd Sub\n\
             Class Foo\n    Public Sub Bar_Hit\n    End Sub\nEnd Class\n",
        );
        vpx.gameitems.push(GameItemEnum::Wall(Wall {
            name: "Bumper1".to_string(),
            ..Wall::default()
        }));
        vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
        vpx.gamedata.name = "Table1".to_string();
        let findings = script_findings(&vpx);
        assert_eq!(
            findings,
            vec![
                Kind::HandlerWithoutItem {
                    name: "Arch1_Hit".to_string(),
                    location: at(4, 5),
                },
                Kind::HandlerWithoutItem {
                    name: "TBWR_Timer".to_string(),
                    location: at(6, 5),
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
        assert_eq!(
            findings[0].to_string(),
            "script has the event handler \"Arch1_Hit\" for an item that does not exist"
        );
    }

    #[test]
    fn script_names_that_hide_items_are_reported() {
        use crate::vpx::collection::Collection;
        use crate::vpx::gameitem::wall::Wall;
        let mut vpx = scripted(
            "Dim Bumper1, Free\n\
             Const Wall1 = 3\n\
             Public Wall2\n\
             Sub AllLights\nEnd Sub\n\
             Class Wall3\nEnd Class\n\
             Sub Table1_Init\n    Dim Wall4\nEnd Sub\n",
        );
        for name in ["Bumper1", "Wall1", "Wall2", "Wall3", "Wall4"] {
            vpx.gameitems.push(GameItemEnum::Wall(Wall {
                name: name.to_string(),
                ..Wall::default()
            }));
        }
        vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
        vpx.collections.push(Collection {
            name: "AllLights".to_string(),
            items: Vec::new(),
            fire_events: false,
            stop_single_events: false,
            group_elements: false,
        });
        vpx.gamedata.collections_size = vpx.collections.len() as u32;
        let shadows =
            |name: &str, kind: NameKind, location: ScriptLocation| Kind::ScriptNameShadowsItem {
                name: name.to_string(),
                kind,
                location,
            };
        // located where the script declares the name, whatever declares it
        assert_eq!(
            script_findings(&vpx),
            vec![
                shadows("Bumper1", NameKind::GameItem, at(2, 5)),
                shadows("Wall1", NameKind::GameItem, at(3, 7)),
                shadows("Wall2", NameKind::GameItem, at(4, 8)),
                shadows("AllLights", NameKind::Collection, at(5, 5)),
                shadows("Wall3", NameKind::GameItem, at(7, 7)),
                unused("Bumper1", at(2, 5)),
                unused("Free", at(2, 14)),
                unused("Wall2", at(4, 8)),
                unused_local("Table1_Init.Wall4", at(10, 9)),
            ]
        );
    }

    #[test]
    fn rnd_without_randomize_is_a_suggestion() {
        let vpx = scripted("Sub Table1_Init\n    x = Rnd * 10\nEnd Sub\n");
        assert_eq!(script_findings(&vpx), vec![Kind::RndWithoutRandomize]);
        assert_eq!(Kind::RndWithoutRandomize.severity(), Severity::Suggestion);
        let vpx = scripted("Randomize\nSub Table1_Init\n    x = Rnd * 10\nEnd Sub\n");
        assert_eq!(script_findings(&vpx), vec![]);
    }

    #[test]
    fn a_static_primitive_named_in_the_script_is_reported() {
        use crate::vpx::gameitem::primitive::Primitive;
        let mut vpx = scripted(
            "Sub Table1_Init\r\n    Baked.Visible = False\r\n    Dynamic.Visible = False\r\nEnd Sub\r\n",
        );
        let primitive = |name: &str, static_rendering: bool| {
            GameItemEnum::Primitive(Box::new(Primitive {
                name: name.to_string(),
                static_rendering,
                ..Primitive::default()
            }))
        };
        vpx.gameitems.push(primitive("Baked", true));
        vpx.gameitems.push(primitive("Dynamic", false));
        vpx.gameitems.push(primitive("Unmentioned", true));
        vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
        let findings = script_findings(&vpx);
        assert_eq!(
            findings,
            vec![Kind::StaticPrimitiveInScript {
                name: "Baked".to_string(),
                item: "Primitive \"Baked\"".to_string(),
                script_toggles_prerendering: false,
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Warning);
    }

    #[test]
    fn a_script_that_toggles_static_prerendering_is_only_informed() {
        use crate::vpx::gameitem::primitive::Primitive;
        let mut vpx = scripted(
            "Sub Table1_OptionEvent(ByVal eventId)\r\n    If eventId = 1 Then DisableStaticPreRendering = True\r\n    Baked.ReflectionEnabled = False\r\nEnd Sub\r\n",
        );
        vpx.gameitems
            .push(GameItemEnum::Primitive(Box::new(Primitive {
                name: "Baked".to_string(),
                static_rendering: true,
                ..Primitive::default()
            })));
        vpx.gamedata.gameitems_size = vpx.gameitems.len() as u32;
        let findings = script_findings(&vpx);
        assert_eq!(
            findings,
            vec![Kind::StaticPrimitiveInScript {
                name: "Baked".to_string(),
                item: "Primitive \"Baked\"".to_string(),
                script_toggles_prerendering: true,
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
        assert_eq!(
            findings[0].to_string(),
            "Primitive \"Baked\": is static (baked at load) and the script refers to it; the script toggles DisableStaticPrerendering, so writes to its properties land while that is set or before the first frame"
        );
    }

    #[test]
    fn a_clean_script_has_no_script_findings() {
        let vpx = scripted("Sub Foo()\nEnd Sub\n");
        assert_eq!(script_findings(&vpx), Vec::new());
    }

    #[test]
    fn assigning_a_ball_id_is_an_error() {
        let vpx = scripted(
            "Sub Tag(ball)
    ball.ID = 5
    With ball
        .Id = 2
    End With
                 If ball.ID = 3 Then Tag = ball.ID
End Sub
",
        );
        let findings = script_findings(&vpx);
        assert_eq!(
            findings,
            vec![
                Kind::BallIdAssigned {
                    location: at(3, 10)
                },
                Kind::BallIdAssigned {
                    location: at(5, 10)
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Error);
        assert_eq!(
            findings[0].to_string(),
            "script assigns a ball's ID, which is read only; use UserValue instead"
        );
    }

    #[test]
    fn assigning_the_id_of_a_script_class_instance_is_not_a_ball_id() {
        for class in [
            "Class Tracked\n    Public ID\nEnd Class\n",
            "Class Tracked\n    Dim id\nEnd Class\n",
            "Class Tracked\n    Private m\n    Property Let ID(v)\n        m = v\n    End Property\nEnd Class\n",
        ] {
            let vpx = scripted(&format!(
                "{class}Dim t, all(1)\nSet t = New Tracked\nSet all(0) = New Tracked\n\
                 t.ID = 5\nall(0).ID = 6\nWith t\n    .ID = 7\nEnd With\n"
            ));
            assert_eq!(script_findings(&vpx), Vec::new(), "{class}");
        }
        let vpx = scripted(
            "Class Tracked\n    Public ID\n    Sub Tag\n        Me.ID = 5\n    End Sub\nEnd Class\n",
        );
        assert_eq!(script_findings(&vpx), Vec::new());
    }

    #[test]
    fn a_class_with_an_id_does_not_hide_a_ball_id_assignment() {
        // nFozzy's spoofball, copied into many tables, keeps a ball's ID
        // in a class of its own
        let vpx = scripted(
            "Class spoofball\n    Public ID\nEnd Class\nClass ReadOnly\n    Property Get ID\n        ID = 1\n    End Property\nEnd Class\n\
             Dim CageBall\nSet CageBall = Kicker1.CreateBall\nCageBall.ID = 1000\n",
        );
        assert_eq!(
            script_findings(&vpx),
            vec![Kind::BallIdAssigned {
                location: at(12, 10)
            }]
        );
    }

    #[test]
    fn a_missing_option_explicit_is_a_suggestion() {
        let mut vpx = clean_vpx();
        vpx.gamedata.code.string = "Sub Foo()\r\nEnd Sub\r\n".to_string();
        let findings = script_findings(&vpx);
        assert_eq!(findings, vec![Kind::MissingOptionExplicit]);
        assert_eq!(findings[0].severity(), Severity::Suggestion);
    }

    #[test]
    fn a_duplicate_procedure_is_reported() {
        let vpx = scripted("Sub Foo()\nEnd Sub\nSub Foo()\nEnd Sub\n");
        assert_eq!(
            script_findings(&vpx),
            vec![Kind::DuplicateProcedure {
                name: "Foo".to_string(),
                location: at(4, 5),
            }]
        );
    }

    #[test]
    fn a_duplicate_procedure_is_located_at_the_name_of_its_second_declaration() {
        // not at the first one, not at a function of another name, and at the name
        // instead of the `Private Sub` before it
        let mut vpx = scripted(
            "Sub Reset\nEnd Sub\nFunction Other\nEnd Function\n\
             Private Sub Reset\nEnd Sub\n\
             Sub Table1_Init\n    Reset\n    x = Other\nEnd Sub\n",
        );
        vpx.gamedata.name = "Table1".to_string();
        assert_eq!(
            script_findings(&vpx),
            vec![Kind::DuplicateProcedure {
                name: "Reset".to_string(),
                location: at(6, 13),
            }]
        );
    }

    #[test]
    fn a_method_name_reused_across_classes_is_not_a_duplicate() {
        let vpx = scripted(
            "Class A\nPublic Sub Init()\nEnd Sub\nEnd Class\n                 Class B\nPublic Sub Init()\nEnd Sub\nEnd Class\n",
        );
        assert_eq!(script_findings(&vpx), Vec::new());
    }

    #[test]
    fn execute_is_flagged_but_execute_global_is_not() {
        let executed = scripted("Execute \"x = 1\"\n");
        assert_eq!(script_findings(&executed), vec![Kind::ExecuteUsed]);
        let global = scripted("ExecuteGlobal \"x = 1\"\n");
        assert_eq!(script_findings(&global), Vec::new());
    }

    #[test]
    fn a_broken_script_reports_a_parse_error() {
        let mut vpx = clean_vpx();
        vpx.gamedata.code.string = "Sub Foo(\r\n".to_string();
        let findings = script_findings(&vpx);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        let Kind::ScriptParseError { detail, location } = &findings[0] else {
            panic!("{findings:#?}");
        };
        // the position is in the location, not repeated in the text
        assert!(location.is_some(), "{findings:#?}");
        assert!(!detail.contains("line "), "{detail}");
    }
}
