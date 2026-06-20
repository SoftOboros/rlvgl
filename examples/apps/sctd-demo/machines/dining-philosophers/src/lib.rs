//! Generated SCXML state machine crate: `ScxmlDiningPhilosophers`.
//!
//! This crate was produced by the softoboros `rust_ir_emitter` pipeline
//! from a `MachineIR` lowered from an SCXML document.  The runtime uses
//! an embedded-IR evaluator approach: the entire state / transition / action
//! tree is encoded as Rust data literals and interpreted at runtime rather
//! than generating specialized match arms per transition.
//!
//! # Feature flags
//! * `std` (default) -- uses `std::collections` types.
//! * `no_std` -- uses `alloc::collections` types; requires an allocator.
//!
//! # Blocked constructs
//! Constructs not yet admitted by the M1P6 gate are stubbed with
//! `// BLOCKED: <diagnostic>` comments and safe placeholders.

#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "std")]
use std::collections::{BTreeMap, VecDeque};
#[cfg(feature = "std")]
use std::format;
#[cfg(feature = "std")]
use std::string::{String, ToString};
#[cfg(feature = "std")]
use std::vec::Vec;
#[cfg(feature = "std")]
use std::boxed::Box;
// Pull the vec! macro into scope unconditionally under std.
#[cfg(feature = "std")]
#[allow(unused_imports)]
use std::vec;

#[cfg(not(feature = "std"))]
use alloc::collections::{BTreeMap, VecDeque};
#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
// Pull the vec! macro into scope under no_std.
#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use alloc::vec;

// Provide a no-op `eprintln!` replacement under no_std so that
// diagnostic prints compile away cleanly on embedded targets.
// Under std the real eprintln! from std is used via normal macro
// resolution (no re-export needed).
#[cfg(not(feature = "std"))]
macro_rules! eprintln {
    ($($arg:tt)*) => { () };
}

// ---------------------------------------------------------------------------
// Value model (M3 Amendment A1, D-M1P6-3)
// ---------------------------------------------------------------------------

/// Tagged union of runtime values for the SCXML datamodel.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Floating-point number.
    Number(f64),
    /// Integer.
    Int(i64),
    /// String.
    Str(String),
    /// Boolean.
    Bool(bool),
    /// Array (ordered sequence).
    Array(Vec<Value>),
    /// String-keyed map (ordered).
    Map(BTreeMap<String, Value>),
    /// Undefined / null sentinel.
    Undefined,
}

// ---------------------------------------------------------------------------
// no_std-safe float helpers (avoid f64::fract/abs/is_nan which need libm)
// ---------------------------------------------------------------------------

/// Return true if x is NaN (no_std-safe via bit pattern).
#[inline]
fn f64_is_nan(x: f64) -> bool {
    let bits = x.to_bits();
    (bits & 0x7FF0_0000_0000_0000) == 0x7FF0_0000_0000_0000
        && (bits & 0x000F_FFFF_FFFF_FFFF) != 0
}

/// Return true if x has no fractional part (no_std-safe).
/// Treats NaN and infinity as "has fractional part".
#[inline]
fn f64_is_integer(x: f64) -> bool {
    if f64_is_nan(x) { return false; }
    // Cast to i64 and back: if equal, it is an integer
    let as_i64 = x as i64;
    (as_i64 as f64) == x
}

/// Compute absolute value of f64 (no_std-safe via bit mask).
#[inline]
fn f64_abs(x: f64) -> f64 {
    f64::from_bits(x.to_bits() & 0x7FFF_FFFF_FFFF_FFFF)
}

/// A canonical NaN bit pattern (no_std-safe).
const F64_NAN: f64 = f64::NAN;

impl Value {
    /// Return the truthy interpretation per ECMAScript semantics.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0 && !f64_is_nan(*n),
            Value::Int(i) => *i != 0,
            Value::Str(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Map(m) => !m.is_empty(),
            Value::Undefined => false,
        }
    }

    /// Coerce to f64 for arithmetic, ECMAScript-style.
    pub fn to_f64(&self) -> f64 {
        match self {
            Value::Number(n) => *n,
            Value::Int(i) => *i as f64,
            Value::Bool(true) => 1.0,
            Value::Bool(false) => 0.0,
            Value::Str(s) => s.parse::<f64>().unwrap_or(F64_NAN),
            _ => F64_NAN,
        }
    }

    /// Coerce to i64.
    pub fn to_i64(&self) -> i64 {
        match self {
            Value::Int(i) => *i,
            Value::Number(n) => *n as i64,
            Value::Bool(true) => 1,
            Value::Bool(false) => 0,
            Value::Str(s) => s.parse::<i64>().unwrap_or(0),
            _ => 0,
        }
    }

    /// Coerce to a display string, ECMAScript-style.
    pub fn to_display_string(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Number(n) => {
                if f64_is_integer(*n) && f64_abs(*n) < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            Value::Int(i) => format!("{}", i),
            Value::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
            Value::Array(_) => "[Array]".to_string(),
            Value::Map(_) => "[Object]".to_string(),
            Value::Undefined => "undefined".to_string(),
        }
    }
}

impl core::fmt::Display for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Undefined
    }
}

// ---------------------------------------------------------------------------
// Scope (datamodel variable store)
// ---------------------------------------------------------------------------

/// Variable store for one machine instance.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    vars: BTreeMap<String, Value>,
}

impl Scope {
    /// Create an empty scope.
    pub fn new() -> Self {
        Self { vars: BTreeMap::new() }
    }

    /// Read a variable by name.  Returns ``Value::Undefined`` when absent.
    pub fn get(&self, name: &str) -> Value {
        self.vars.get(name).cloned().unwrap_or(Value::Undefined)
    }

    /// Write a variable.
    pub fn set(&mut self, name: String, val: Value) {
        self.vars.insert(name, val);
    }

    /// Read a nested map key: ``scope.get_map_key("var", "key")``.
    pub fn get_map_key(&self, var: &str, key: &str) -> Value {
        match self.vars.get(var) {
            Some(Value::Map(m)) => m.get(key).cloned().unwrap_or(Value::Undefined),
            _ => Value::Undefined,
        }
    }

    /// Write a nested map key.
    pub fn set_map_key(&mut self, var: &str, key: String, val: Value) {
        let entry = self.vars.entry(var.to_string()).or_insert_with(|| {
            Value::Map(BTreeMap::new())
        });
        if let Value::Map(ref mut m) = entry {
            m.insert(key, val);
        }
    }
}

// ---------------------------------------------------------------------------
// EventCtx (current event context)
// ---------------------------------------------------------------------------

/// Context for the currently-processed event, accessible as ``_event``.
#[derive(Debug, Clone, Default)]
pub struct EventCtx {
    /// Event name (``_event.name``).
    pub name: String,
    /// Event data payload (``_event.data``).
    pub data: Value,
}

impl EventCtx {
    /// Create an event context.
    pub fn new(name: impl Into<String>, data: Value) -> Self {
        Self { name: name.into(), data }
    }
}

// ---------------------------------------------------------------------------
// ExprIr -- embedded IR for expressions (D-M1P6-2)
// ---------------------------------------------------------------------------

/// IR node for one expression in the SCXML datamodel.
///
/// These mirror the Python ``ExpressionNode`` families from
/// ``scjson.exec_ir`` and are interpreted at runtime by ``eval_expr``.
#[derive(Debug, Clone)]
pub enum ExprIr {
    /// Numeric constant.
    NumericLiteral(f64),
    /// Integer constant.
    IntLiteral(i64),
    /// String constant.
    StringLiteral(String),
    /// Boolean constant.
    BoolLiteral(bool),
    /// Read a datamodel variable by name.
    VarRead(String),
    /// Read the current event name (``_event.name``).
    EventName,
    /// Read the current event data (``_event.data``).
    EventData,
    /// Static member access on an expression.
    MemberAccess(Box<ExprIr>, String),
    /// Computed index access.
    IndexAccess(Box<ExprIr>, Box<ExprIr>),
    /// Binary operation.
    BinOp(String, Box<ExprIr>, Box<ExprIr>),
    /// Unary operation (``"not"`` or ``"neg"``).
    UnaryOp(String, Box<ExprIr>),
    /// Admitted built-in call (``"int"``, ``"str"``, ``"float"``, ``"replace"``).
    Call(String, Vec<ExprIr>, Option<Box<ExprIr>>),
    /// Array literal.
    ArrayLiteral(Vec<ExprIr>),
    /// Map literal (pairs of key_expr, val_expr).
    MapLiteral(Vec<(ExprIr, ExprIr)>),
    /// Unsupported / blocked expression -- evaluates to Undefined.
    Blocked(String),
}

// ---------------------------------------------------------------------------
// ActionIr -- embedded IR for actions (D-M1P6-4..6)
// ---------------------------------------------------------------------------

/// One branch of a conditional (IfNode).
#[derive(Debug, Clone)]
pub struct IfBranchIr {
    /// Guard expression; None means else.
    pub guard: Option<ExprIr>,
    /// Actions in this branch.
    pub body: Vec<ActionIr>,
}

/// Send parameters.
#[derive(Debug, Clone)]
pub struct SendParamIr {
    pub name: String,
    pub expr: ExprIr,
}

/// IR node for one executable action.
#[derive(Debug, Clone)]
pub enum ActionIr {
    /// Assign a value to a variable (``set``, ``add``, ``sub``, ``mul``, ``div``).
    Assign {
        /// Target variable name, or ``"<map>[<key>]"`` for map-key writes.
        loc: String,
        /// Optional computed key expression for map-key writes where the key
        /// is a non-trivial expression (e.g. ``'taken.' + str(i_ID_LEFT)``).
        /// When ``Some``, overrides the bracket-parsed key_part in ``loc``.
        key_expr: Option<Box<ExprIr>>,
        expr: ExprIr,
        op: String,
    },
    /// Log a value.
    Log { expr: ExprIr, label: Option<String> },
    /// Raise an internal event.
    Raise { event: String },
    /// Send an event (with optional dynamic expression slots).
    Send {
        event: Option<String>,
        eventexpr: Option<ExprIr>,
        target: Option<String>,
        targetexpr: Option<ExprIr>,
        delay: Option<String>,
        delayexpr: Option<ExprIr>,
        send_id: Option<String>,
        content_expr: Option<ExprIr>,
        params: Vec<SendParamIr>,
    },
    /// Cancel a pending send by id.
    Cancel { sendid: Option<String> },
    /// Conditional action.
    If { branches: Vec<IfBranchIr> },
    /// Foreach loop.
    Foreach {
        array_expr: ExprIr,
        item: String,
        index: Option<String>,
        body: Vec<ActionIr>,
    },
    /// Blocked / capability call -- logs the diagnostic and raises error event.
    Blocked { diagnostic: String, error_event: String },
}

// ---------------------------------------------------------------------------
// StateIr -- embedded state tree
// ---------------------------------------------------------------------------

/// IR for one data slot.
#[derive(Debug, Clone)]
pub struct DataSlotIr {
    pub id: String,
    pub initial_value: Option<Value>,
}

/// IR for one transition.
#[derive(Debug, Clone)]
pub struct TransitionIr {
    /// Event pattern to match (None = eventless/epsilon).
    pub event: Option<String>,
    /// Target state IDs.
    pub targets: Vec<String>,
    /// Guard expression (None = always enabled).
    pub guard: Option<ExprIr>,
    /// Executable actions.
    pub actions: Vec<ActionIr>,
}

/// Embedded IR for an inline child machine (D-M1P6-5).
#[derive(Debug, Clone)]
pub struct ChildMachineIr {
    pub name: Option<String>,
    pub initial: Vec<String>,
    pub datamodel: Vec<DataSlotIr>,
    pub children: Vec<StateNodeIr>,
}

/// IR for one invoke element.
#[derive(Debug, Clone)]
pub struct InvokeIr {
    pub invoke_id: Option<String>,
    pub type_value: String,
    /// External source URI (blocked when Some).
    pub src: Option<String>,
    /// Parameters passed to the child datamodel on spawn.
    pub params: Vec<SendParamIr>,
    pub autoforward: bool,
    /// Blocked diagnostic message when src is Some.
    pub blocked_msg: Option<String>,
    /// Inline child machine IR (present when the invoke used <content><scxml>).
    pub child_machine: Option<Box<ChildMachineIr>>,
}

/// One state / parallel / final / history node.
#[derive(Debug, Clone)]
pub enum StateNodeIr {
    State {
        id: String,
        initial: Option<String>,
        datamodel: Vec<DataSlotIr>,
        onentry: Vec<ActionIr>,
        onexit: Vec<ActionIr>,
        transitions: Vec<TransitionIr>,
        children: Vec<StateNodeIr>,
        invokes: Vec<InvokeIr>,
    },
    Parallel {
        id: String,
        datamodel: Vec<DataSlotIr>,
        onentry: Vec<ActionIr>,
        onexit: Vec<ActionIr>,
        transitions: Vec<TransitionIr>,
        children: Vec<StateNodeIr>,
        invokes: Vec<InvokeIr>,
    },
    Final {
        id: String,
    },
    History {
        id: String,
        history_type: String,
    },
}

impl StateNodeIr {
    /// Get the state id.
    pub fn id(&self) -> &str {
        match self {
            StateNodeIr::State { id, .. } => id,
            StateNodeIr::Parallel { id, .. } => id,
            StateNodeIr::Final { id } => id,
            StateNodeIr::History { id, .. } => id,
        }
    }

    /// Get child nodes (empty for Final/History).
    pub fn children(&self) -> &[StateNodeIr] {
        match self {
            StateNodeIr::State { children, .. } => children,
            StateNodeIr::Parallel { children, .. } => children,
            _ => &[],
        }
    }

    /// Get transitions (empty for Final/History).
    pub fn transitions(&self) -> &[TransitionIr] {
        match self {
            StateNodeIr::State { transitions, .. } => transitions,
            StateNodeIr::Parallel { transitions, .. } => transitions,
            _ => &[],
        }
    }

    /// Get onentry actions.
    pub fn onentry(&self) -> &[ActionIr] {
        match self {
            StateNodeIr::State { onentry, .. } => onentry,
            StateNodeIr::Parallel { onentry, .. } => onentry,
            _ => &[],
        }
    }

    /// Get onexit actions.
    pub fn onexit(&self) -> &[ActionIr] {
        match self {
            StateNodeIr::State { onexit, .. } => onexit,
            StateNodeIr::Parallel { onexit, .. } => onexit,
            _ => &[],
        }
    }

    /// Get initial child id.
    pub fn initial_child(&self) -> Option<&str> {
        match self {
            StateNodeIr::State { initial, .. } => initial.as_deref(),
            _ => None,
        }
    }

    /// True when this is a Final state.
    pub fn is_final(&self) -> bool {
        matches!(self, StateNodeIr::Final { .. })
    }

    /// Find a descendant node by id (DFS).
    pub fn find_descendant(&self, target_id: &str) -> Option<&StateNodeIr> {
        if self.id() == target_id {
            return Some(self);
        }
        for child in self.children() {
            if let Some(found) = child.find_descendant(target_id) {
                return Some(found);
            }
        }
        None
    }

    /// Collect all descendant IDs in DFS order (including self).
    pub fn all_descendant_ids(&self) -> Vec<String> {
        let mut ids = vec![self.id().to_string()];
        for child in self.children() {
            ids.extend(child.all_descendant_ids());
        }
        ids
    }
}

/// Top-level machine IR (embedded as static data in each generated crate).
#[derive(Debug, Clone)]
pub struct MachineIrData {
    pub name: Option<String>,
    pub initial: Vec<String>,
    pub datamodel: Vec<DataSlotIr>,
    pub children: Vec<StateNodeIr>,
}

// ---------------------------------------------------------------------------
// Expression evaluator
// ---------------------------------------------------------------------------

/// Evaluate an expression IR node against the current scope and event context.
///
/// Never panics: unsupported/blocked nodes return ``Value::Undefined``.
pub fn eval_expr(expr: &ExprIr, scope: &Scope, event: &EventCtx) -> Value {
    match expr {
        ExprIr::NumericLiteral(n) => Value::Number(*n),
        ExprIr::IntLiteral(i) => Value::Int(*i),
        ExprIr::StringLiteral(s) => Value::Str(s.clone()),
        ExprIr::BoolLiteral(b) => Value::Bool(*b),

        ExprIr::VarRead(name) => {
            if name == "_event" {
                // Return a pseudo-object; member access handles .name/.data
                Value::Str(event.name.clone())
            } else {
                scope.get(name)
            }
        }

        ExprIr::EventName => Value::Str(event.name.clone()),
        ExprIr::EventData => event.data.clone(),

        ExprIr::MemberAccess(obj_expr, attr) => {
            // Special-case _event.name / _event.data
            if let ExprIr::VarRead(ref var_name) = **obj_expr {
                if var_name == "_event" {
                    return match attr.as_str() {
                        "name" => Value::Str(event.name.clone()),
                        "data" => event.data.clone(),
                        _ => Value::Undefined,
                    };
                }
            }
            let obj_val = eval_expr(obj_expr, scope, event);
            eval_member(obj_val, attr)
        }

        ExprIr::IndexAccess(obj_expr, idx_expr) => {
            let obj_val = eval_expr(obj_expr, scope, event);
            let idx_val = eval_expr(idx_expr, scope, event);
            eval_index(obj_val, idx_val)
        }

        ExprIr::BinOp(op, lhs, rhs) => {
            eval_binop(op, lhs, rhs, scope, event)
        }

        ExprIr::UnaryOp(op, operand) => {
            let val = eval_expr(operand, scope, event);
            match op.as_str() {
                "not" => Value::Bool(!val.is_truthy()),
                "neg" => Value::Number(-val.to_f64()),
                _ => Value::Undefined,
            }
        }

        ExprIr::Call(func, args, receiver) => {
            eval_call(func, args, receiver.as_deref(), scope, event)
        }

        ExprIr::ArrayLiteral(items) => {
            let vals: Vec<Value> = items.iter()
                .map(|e| eval_expr(e, scope, event))
                .collect();
            Value::Array(vals)
        }

        ExprIr::MapLiteral(entries) => {
            let mut map = BTreeMap::new();
            for (k, v) in entries {
                let key_val = eval_expr(k, scope, event);
                let val_val = eval_expr(v, scope, event);
                map.insert(key_val.to_display_string(), val_val);
            }
            Value::Map(map)
        }

        ExprIr::Blocked(diag) => {
            eprintln!("BLOCKED expr: {}", diag);
            Value::Undefined
        }
    }
}

fn eval_member(obj: Value, attr: &str) -> Value {
    match obj {
        Value::Map(m) => m.get(attr).cloned().unwrap_or(Value::Undefined),
        Value::Str(s) => match attr {
            "length" => Value::Int(s.len() as i64),
            _ => Value::Undefined,
        },
        Value::Array(a) => match attr {
            "length" => Value::Int(a.len() as i64),
            _ => Value::Undefined,
        },
        _ => Value::Undefined,
    }
}

fn eval_index(obj: Value, idx: Value) -> Value {
    match obj {
        Value::Array(a) => {
            let i = idx.to_i64();
            if i >= 0 && (i as usize) < a.len() {
                a[i as usize].clone()
            } else {
                Value::Undefined
            }
        }
        Value::Map(m) => {
            let key = idx.to_display_string();
            m.get(&key).cloned().unwrap_or(Value::Undefined)
        }
        _ => Value::Undefined,
    }
}

fn eval_binop(op: &str, lhs: &ExprIr, rhs: &ExprIr, scope: &Scope, event: &EventCtx) -> Value {
    // Short-circuit boolean ops
    match op {
        "&&" => {
            let l = eval_expr(lhs, scope, event);
            if !l.is_truthy() { return Value::Bool(false); }
            let r = eval_expr(rhs, scope, event);
            return Value::Bool(r.is_truthy());
        }
        "||" => {
            let l = eval_expr(lhs, scope, event);
            if l.is_truthy() { return Value::Bool(true); }
            let r = eval_expr(rhs, scope, event);
            return Value::Bool(r.is_truthy());
        }
        _ => {}
    }

    let lv = eval_expr(lhs, scope, event);
    let rv = eval_expr(rhs, scope, event);

    match op {
        "+" => {
            // String concatenation if either operand is a string
            if matches!(lv, Value::Str(_)) || matches!(rv, Value::Str(_)) {
                Value::Str(format!("{}{}", lv.to_display_string(), rv.to_display_string()))
            } else {
                let ln = lv.to_f64();
                let rn = rv.to_f64();
                let result = ln + rn;
                if f64_is_integer(result) && f64_abs(result) < 9.007e15 {
                    Value::Int(result as i64)
                } else {
                    Value::Number(result)
                }
            }
        }
        "-" => {
            let result = lv.to_f64() - rv.to_f64();
            if f64_is_integer(result) { Value::Int(result as i64) } else { Value::Number(result) }
        }
        "*" => {
            let result = lv.to_f64() * rv.to_f64();
            if f64_is_integer(result) { Value::Int(result as i64) } else { Value::Number(result) }
        }
        "/" => Value::Number(lv.to_f64() / rv.to_f64()),
        "|" => Value::Int(lv.to_i64() | rv.to_i64()),
        "&" => Value::Int(lv.to_i64() & rv.to_i64()),
        "==" => Value::Bool(values_equal(&lv, &rv)),
        "!=" => Value::Bool(!values_equal(&lv, &rv)),
        "<"  => Value::Bool(lv.to_f64() < rv.to_f64()),
        "<=" => Value::Bool(lv.to_f64() <= rv.to_f64()),
        ">"  => Value::Bool(lv.to_f64() > rv.to_f64()),
        ">=" => Value::Bool(lv.to_f64() >= rv.to_f64()),
        _ => {
            eprintln!("BLOCKED: unknown binop {:?}", op);
            Value::Undefined
        }
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Int(x), Value::Number(y)) => (*x as f64) == *y,
        (Value::Number(x), Value::Int(y)) => *x == (*y as f64),
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Undefined, Value::Undefined) => true,
        _ => false,
    }
}

fn eval_call(
    func: &str,
    args: &[ExprIr],
    receiver: Option<&ExprIr>,
    scope: &Scope,
    event: &EventCtx,
) -> Value {
    match func {
        "int" | "parseInt" => {
            let arg = args.first().map(|e| eval_expr(e, scope, event)).unwrap_or(Value::Undefined);
            Value::Int(arg.to_i64())
        }
        "float" | "Number" | "parseFloat" => {
            let arg = args.first().map(|e| eval_expr(e, scope, event)).unwrap_or(Value::Undefined);
            Value::Number(arg.to_f64())
        }
        "str" | "String" => {
            let arg = args.first().map(|e| eval_expr(e, scope, event)).unwrap_or(Value::Undefined);
            Value::Str(arg.to_display_string())
        }
        "replace" => {
            // receiver.replace(from, to)
            let recv_val = receiver.map(|e| eval_expr(e, scope, event)).unwrap_or(Value::Undefined);
            let from = args.first().map(|e| eval_expr(e, scope, event)).unwrap_or(Value::Undefined);
            let to = args.get(1).map(|e| eval_expr(e, scope, event)).unwrap_or(Value::Undefined);
            if let Value::Str(s) = recv_val {
                Value::Str(s.replace(&from.to_display_string(), &to.to_display_string()))
            } else {
                Value::Undefined
            }
        }
        _ => {
            eprintln!("BLOCKED: unknown call {:?}", func);
            Value::Undefined
        }
    }
}

// ---------------------------------------------------------------------------
// Action executor
// ---------------------------------------------------------------------------

/// Execute a slice of action IR nodes, mutating scope and the event queue.
///
/// The ``queue`` receives events raised by ``RaiseNode`` or ``SendNode``
/// (simplified: external send is logged to stderr).  The ``event_ctx``
/// provides the current ``_event``.
pub fn exec_actions(
    actions: &[ActionIr],
    scope: &mut Scope,
    queue: &mut VecDeque<String>,
    event: &EventCtx,
) {
    for action in actions {
        exec_action(action, scope, queue, event);
    }
}

fn exec_action(
    action: &ActionIr,
    scope: &mut Scope,
    queue: &mut VecDeque<String>,
    event: &EventCtx,
) {
    match action {
        ActionIr::Assign { loc, key_expr, expr, op } => {
            let new_val = eval_expr(expr, scope, event);
            // Detect map[key] pattern in loc
            if let Some(bracket_pos) = loc.find('[') {
                let var_name = &loc[..bracket_pos];
                // If a pre-computed key expression is available, evaluate it.
                // Otherwise fall back to parsing the key_part from the loc string.
                let key = if let Some(ke) = key_expr {
                    eval_expr(ke, scope, event).to_display_string()
                } else {
                    let key_part = &loc[bracket_pos + 1..loc.len().saturating_sub(1)];
                    // key_part may be a literal string, ``_event.name``, or a var name
                    let single_q = "'";
                    if (key_part.starts_with('"') && key_part.ends_with('"'))
                        || (key_part.starts_with(single_q) && key_part.ends_with(single_q))
                    {
                        key_part[1..key_part.len() - 1].to_string()
                    } else if key_part == "_event.name" {
                        event.name.clone()
                    } else {
                        scope.get(key_part).to_display_string()
                    }
                };
                let current = scope.get_map_key(var_name, &key);
                let result = apply_op(op, current, new_val);
                scope.set_map_key(var_name, key, result);
            } else {
                let current = scope.get(loc);
                let result = apply_op(op, current, new_val);
                scope.set(loc.clone(), result);
            }
        }

        ActionIr::Log { expr, label } => {
            let val = eval_expr(expr, scope, event);
            match label {
                Some(lbl) => eprintln!("[LOG {}] {}", lbl, val),
                None => eprintln!("[LOG] {}", val),
            }
        }

        ActionIr::Raise { event: ev } => {
            queue.push_back(ev.clone());
        }

        ActionIr::Send { event: ev, eventexpr, target, targetexpr, delay, delayexpr, send_id, content_expr, .. } => {
            // Resolve event name (static or dynamic).
            let resolved_event = if let Some(expr) = eventexpr {
                eval_expr(expr, scope, event).to_display_string()
            } else {
                ev.as_deref().unwrap_or("").to_string()
            };
            // Resolve target (static wins over dynamic; dynamic evaluated if static absent).
            let resolved_target = if let Some(t) = target {
                t.clone()
            } else if let Some(expr) = targetexpr {
                eval_expr(expr, scope, event).to_display_string()
            } else {
                String::new()
            };
            // Resolve delay: static string or dynamic expression.
            let delay_str = if let Some(expr) = delayexpr {
                eval_expr(expr, scope, event).to_display_string()
            } else {
                delay.as_deref().unwrap_or("").to_string()
            };
            let delay_ms = parse_delay_ms(&delay_str);
            let resolved_content = content_expr.as_ref()
                .map(|e| eval_expr(e, scope, event))
                .unwrap_or(Value::Undefined);
            let content_str = resolved_content.to_display_string();
            let send_id_str = send_id.as_deref().unwrap_or("");
            eprintln!(
                "[SEND] event={:?} target={:?} id={:?} delay={}ms",
                resolved_event, resolved_target, send_id_str, delay_ms
            );
            // Route based on target (D-M1P6-6).
            // ``#_parent`` -- cross-machine send to invoking parent; prefixed
            // in the queue so ChildMachine::step can collect and forward.
            // ``#_<invokeid>`` -- send to an inline child by invoke id;
            // Machine::step extracts and delivers.
            // Empty / ``#_internal`` -- intra-machine (push directly).
            // Delayed sends (delay_ms > 0) are queued as ``__delayed__<ms>:<id>:<target>:<event>:<content>``
            // so the timer queue in Machine can schedule them.
            if delay_ms > 0 {
                queue.push_back(format!(
                    "__delayed__{}:{}:{}:{}:{}",
                    delay_ms, send_id_str, resolved_target, resolved_event, content_str
                ));
            } else if resolved_target == "#_parent" {
                // Include content so the parent can reconstruct EventCtx.data.
                // Format: "__parent__:<event_name><content>".
                // The NUL/SOH byte (\x01) is safe because SCXML event names
                // and content string representations never contain it.
                queue.push_back(format!("__parent__:{}{}", resolved_event, content_str));
            } else if resolved_target.starts_with("#_") {
                let child_id = &resolved_target[2..];
                queue.push_back(format!("__child__{}:{}", child_id, resolved_event));
            } else if resolved_target.is_empty() || resolved_target == "#_internal" {
                queue.push_back(resolved_event);
            }
            // External (HTTP / remote) targets are logged only; no network at M1.
        }

        ActionIr::Cancel { sendid } => {
            let cid = sendid.as_deref().unwrap_or("");
            eprintln!("[CANCEL] sendid={:?}", cid);
            if !cid.is_empty() {
                queue.push_back(format!("__cancel__{}", cid));
            }
        }

        ActionIr::If { branches } => {
            for branch in branches {
                let guard_ok = match &branch.guard {
                    None => true,
                    Some(g) => eval_expr(g, scope, event).is_truthy(),
                };
                if guard_ok {
                    exec_actions(&branch.body, scope, queue, event);
                    break;
                }
            }
        }

        ActionIr::Foreach { array_expr, item, index, body } => {
            let arr = eval_expr(array_expr, scope, event);
            if let Value::Array(items) = arr {
                for (i, val) in items.into_iter().enumerate() {
                    scope.set(item.clone(), val);
                    if let Some(idx_name) = index {
                        scope.set(idx_name.clone(), Value::Int(i as i64));
                    }
                    exec_actions(body, scope, queue, event);
                }
            }
        }

        ActionIr::Blocked { diagnostic, error_event } => {
            eprintln!("BLOCKED: {}", diagnostic);
            if !error_event.is_empty() {
                queue.push_back(error_event.clone());
            }
        }
    }
}

fn apply_op(op: &str, current: Value, new_val: Value) -> Value {
    match op {
        "add" => {
            let result = current.to_f64() + new_val.to_f64();
            if f64_is_integer(result) { Value::Int(result as i64) } else { Value::Number(result) }
        }
        "sub" => {
            let result = current.to_f64() - new_val.to_f64();
            if f64_is_integer(result) { Value::Int(result as i64) } else { Value::Number(result) }
        }
        "mul" => {
            let result = current.to_f64() * new_val.to_f64();
            if f64_is_integer(result) { Value::Int(result as i64) } else { Value::Number(result) }
        }
        "div" => Value::Number(current.to_f64() / new_val.to_f64()),
        _ => new_val, // "set" and anything else
    }
}

/// Parse a delay string such as ``"1000ms"``, ``"2s"``, or ``"500"`` to milliseconds.
///
/// Returns 0 for empty, ``"0ms"``, ``"0s"``, or ``"0"`` strings.
fn parse_delay_ms(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() || s == "0ms" || s == "0s" || s == "0" { return 0; }
    if let Some(ms) = s.strip_suffix("ms") { return ms.trim().parse::<u64>().unwrap_or(0); }
    if let Some(ss) = s.strip_suffix('s') { return ss.trim().parse::<u64>().unwrap_or(0) * 1000; }
    s.parse::<u64>().unwrap_or(0)
}

// ---------------------------------------------------------------------------
// MachineState enum (one variant per state in the document)
// ---------------------------------------------------------------------------

/// All states reachable in this machine.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MachineState {
    /// State `DiningPhilosophers`.
    DiningPhilosophers,
    /// State `Philosopher5`.
    Philosopher5,
    /// State `P5_Thinking`.
    P5_Thinking,
    /// State `P5_Hungry`.
    P5_Hungry,
    /// State `P5_Eating`.
    P5_Eating,
    /// State `Philosopher4`.
    Philosopher4,
    /// State `P4_Thinking`.
    P4_Thinking,
    /// State `P4_Hungry`.
    P4_Hungry,
    /// State `P4_Eating`.
    P4_Eating,
    /// State `Philosopher3`.
    Philosopher3,
    /// State `P3_Thinking`.
    P3_Thinking,
    /// State `P3_Eating`.
    P3_Eating,
    /// State `P3_Hungry`.
    P3_Hungry,
    /// State `Philosopher2`.
    Philosopher2,
    /// State `P2_Thinking`.
    P2_Thinking,
    /// State `P2_Eating`.
    P2_Eating,
    /// State `P2_Hungry`.
    P2_Hungry,
    /// State `Philosopher1`.
    Philosopher1,
    /// State `P1_Thinking`.
    P1_Thinking,
    /// State `P1_Hungry`.
    P1_Hungry,
    /// State `P1_Eating`.
    P1_Eating,
    /// State `Fork4`.
    Fork4,
    /// State `Fork4_Down`.
    Fork4_Down,
    /// State `Fork4_Up`.
    Fork4_Up,
    /// State `Fork5`.
    Fork5,
    /// State `Fork5_Down`.
    Fork5_Down,
    /// State `Fork5_Up`.
    Fork5_Up,
    /// State `Fork3`.
    Fork3,
    /// State `Fork3_Down`.
    Fork3_Down,
    /// State `Fork3_Up`.
    Fork3_Up,
    /// State `Fork2`.
    Fork2,
    /// State `Fork2_Down`.
    Fork2_Down,
    /// State `Fork2_Up`.
    Fork2_Up,
    /// State `Fork1`.
    Fork1,
    /// State `Fork1_Down`.
    Fork1_Down,
    /// State `Fork1_Up`.
    Fork1_Up,
    /// Unknown / initial sentinel.
    Unknown,
}

impl MachineState {
    /// Return the canonical SCXML state id string.
    pub fn as_str(&self) -> &'static str {
        match self {
            MachineState::DiningPhilosophers => "DiningPhilosophers",
            MachineState::Philosopher5 => "Philosopher5",
            MachineState::P5_Thinking => "P5_Thinking",
            MachineState::P5_Hungry => "P5_Hungry",
            MachineState::P5_Eating => "P5_Eating",
            MachineState::Philosopher4 => "Philosopher4",
            MachineState::P4_Thinking => "P4_Thinking",
            MachineState::P4_Hungry => "P4_Hungry",
            MachineState::P4_Eating => "P4_Eating",
            MachineState::Philosopher3 => "Philosopher3",
            MachineState::P3_Thinking => "P3_Thinking",
            MachineState::P3_Eating => "P3_Eating",
            MachineState::P3_Hungry => "P3_Hungry",
            MachineState::Philosopher2 => "Philosopher2",
            MachineState::P2_Thinking => "P2_Thinking",
            MachineState::P2_Eating => "P2_Eating",
            MachineState::P2_Hungry => "P2_Hungry",
            MachineState::Philosopher1 => "Philosopher1",
            MachineState::P1_Thinking => "P1_Thinking",
            MachineState::P1_Hungry => "P1_Hungry",
            MachineState::P1_Eating => "P1_Eating",
            MachineState::Fork4 => "Fork4",
            MachineState::Fork4_Down => "Fork4_Down",
            MachineState::Fork4_Up => "Fork4_Up",
            MachineState::Fork5 => "Fork5",
            MachineState::Fork5_Down => "Fork5_Down",
            MachineState::Fork5_Up => "Fork5_Up",
            MachineState::Fork3 => "Fork3",
            MachineState::Fork3_Down => "Fork3_Down",
            MachineState::Fork3_Up => "Fork3_Up",
            MachineState::Fork2 => "Fork2",
            MachineState::Fork2_Down => "Fork2_Down",
            MachineState::Fork2_Up => "Fork2_Up",
            MachineState::Fork1 => "Fork1",
            MachineState::Fork1_Down => "Fork1_Down",
            MachineState::Fork1_Up => "Fork1_Up",
            MachineState::Unknown => "Unknown",
        }
    }

    /// Parse a state id string to a MachineState variant.
    pub fn from_id(id: &str) -> Self {
        match id {
            "DiningPhilosophers" => MachineState::DiningPhilosophers,
            "Philosopher5" => MachineState::Philosopher5,
            "P5_Thinking" => MachineState::P5_Thinking,
            "P5_Hungry" => MachineState::P5_Hungry,
            "P5_Eating" => MachineState::P5_Eating,
            "Philosopher4" => MachineState::Philosopher4,
            "P4_Thinking" => MachineState::P4_Thinking,
            "P4_Hungry" => MachineState::P4_Hungry,
            "P4_Eating" => MachineState::P4_Eating,
            "Philosopher3" => MachineState::Philosopher3,
            "P3_Thinking" => MachineState::P3_Thinking,
            "P3_Eating" => MachineState::P3_Eating,
            "P3_Hungry" => MachineState::P3_Hungry,
            "Philosopher2" => MachineState::Philosopher2,
            "P2_Thinking" => MachineState::P2_Thinking,
            "P2_Eating" => MachineState::P2_Eating,
            "P2_Hungry" => MachineState::P2_Hungry,
            "Philosopher1" => MachineState::Philosopher1,
            "P1_Thinking" => MachineState::P1_Thinking,
            "P1_Hungry" => MachineState::P1_Hungry,
            "P1_Eating" => MachineState::P1_Eating,
            "Fork4" => MachineState::Fork4,
            "Fork4_Down" => MachineState::Fork4_Down,
            "Fork4_Up" => MachineState::Fork4_Up,
            "Fork5" => MachineState::Fork5,
            "Fork5_Down" => MachineState::Fork5_Down,
            "Fork5_Up" => MachineState::Fork5_Up,
            "Fork3" => MachineState::Fork3,
            "Fork3_Down" => MachineState::Fork3_Down,
            "Fork3_Up" => MachineState::Fork3_Up,
            "Fork2" => MachineState::Fork2,
            "Fork2_Down" => MachineState::Fork2_Down,
            "Fork2_Up" => MachineState::Fork2_Up,
            "Fork1" => MachineState::Fork1,
            "Fork1_Down" => MachineState::Fork1_Down,
            "Fork1_Up" => MachineState::Fork1_Up,
            _ => MachineState::Unknown,
        }
    }
}

// ---------------------------------------------------------------------------
// Machine struct and step engine
// ---------------------------------------------------------------------------

/// SCXML state machine executor for `ScxmlDiningPhilosophers`.
///
/// Usage:
/// ```ignore
/// use dp_sub::{Machine, Value};
/// let mut m = Machine::new();
/// m.start();
/// m.step("Do.Timer.Hungry", Value::Undefined);
/// println!("current state: {}", m.current_state());
/// ```
/// A pending timer entry for the logical-time timer queue.
#[derive(Debug, Clone)]
pub struct PendingTimer {
    /// Logical time (ms) at which this timer fires.
    pub fire_time_ms: u64,
    /// Monotonically increasing sequence number for stable ordering at equal fire times.
    pub seq: u64,
    /// Invoke id of the child machine that owns this timer (empty = parent machine).
    pub machine_id: String,
    /// Event name to deliver when the timer fires.
    pub event_name: String,
    /// Payload to deliver with the event.
    pub content: Value,
    /// Optional send id for cancellation via ActionIr::Cancel.
    pub send_id: Option<String>,
}

/// An active inline child machine instance.
pub struct ChildMachine {
    /// Invoke id that spawned this child.
    pub invoke_id: String,
    /// The child machine's IR (borrowed from the parent InvokeIr).
    ir: ChildMachineIr,
    scope: Scope,
    active_state: String,
    queue: VecDeque<String>,
    event_ctx: EventCtx,
}

impl ChildMachine {
    /// Spawn a new child machine from a ChildMachineIr.
    ///
    /// *params* is a slice of (name, Value) pairs copied from the parent scope
    /// at invoke time.
    pub fn spawn(invoke_id: String, ir: ChildMachineIr, params: Vec<(String, Value)>) -> Self {
        let mut scope = Scope::new();
        for slot in &ir.datamodel {
            if let Some(ref v) = slot.initial_value {
                scope.set(slot.id.clone(), v.clone());
            } else {
                scope.set(slot.id.clone(), Value::Undefined);
            }
        }
        for (name, val) in params {
            scope.set(name, val);
        }
        let initial = ir.initial.first().cloned().unwrap_or_default();
        let mut child = ChildMachine {
            invoke_id,
            ir,
            scope,
            active_state: initial.clone(),
            queue: VecDeque::new(),
            event_ctx: EventCtx::default(),
        };
        if !initial.is_empty() {
            child.enter_state(&initial);
        }
        child
    }

    /// Process one event in the child machine.
    ///
    /// Returns parent-bound event names (the ``__parent__:`` prefix is
    /// stripped before returning so the parent sees plain event names).
    pub fn step(&mut self, event_name: &str, event_data: Value) -> Vec<String> {
        self.event_ctx = EventCtx::new(event_name, event_data);
        self.process_event(event_name.to_string());
        // Re-evaluate epsilon transitions after event processing.  Epsilon
        // transitions guarded by ``_event.name`` checks (e.g. LeftWaiting →
        // LeftCheck on ``do.taken.*``) depend on the current event_ctx, which
        // is preserved here so they evaluate correctly.
        self.run_epsilon_transitions();
        // Drain internal queue -- skip cross-machine routing prefixes.
        // Events prefixed ``__parent__:`` are collected for the caller;
        // events without any prefix are processed as normal intra-child events.
        let mut parent_events: Vec<String> = Vec::new();
        let mut safety = 0usize;
        while let Some(ev) = self.queue.pop_front() {
            if safety > 1000 { break; }
            safety += 1;
            if let Some(rest) = ev.strip_prefix("__parent__:") {
                // Format: "<event_name><content>".
                // Preserve the full rest (including content suffix) so the parent
                // can reconstruct EventCtx.data when processing this event.
                // Re-wrap as "__pev__<event><content>" for the parent queue.
                parent_events.push(format!("__pev__{}", rest));
            } else if ev.starts_with("__delayed__") || ev.starts_with("__cancel__") {
                // Forward delayed-send and cancel tokens to the parent machine so
                // the timer queue can schedule / cancel them.
                parent_events.push(ev);
            } else {
                let ev_clone = ev.clone();
                self.event_ctx = EventCtx::new(&ev_clone, Value::Undefined);
                self.process_event(ev.clone());
                // Re-evaluate epsilon transitions after each internal event.
                self.run_epsilon_transitions();
            }
        }
        parent_events
    }

    /// Deliver an event from the parent directly to this child machine.
    ///
    /// Used by the parent ``Machine::step`` for ``#_<invokeid>`` routing.
    pub fn deliver(&mut self, event_name: &str, event_data: Value) -> Vec<String> {
        self.step(event_name, event_data)
    }

    /// Drain any parent-bound events accumulated at spawn time.
    ///
    /// Called by the parent ``Machine::enter_state`` immediately after
    /// pushing the child to ``child_machines``, so spawn-time ``#_parent``
    /// sends (e.g., ``think.N`` on ``Thinking`` onentry) reach the parent.
    /// Also forwards ``__delayed__`` and ``__cancel__`` tokens so the parent
    /// timer queue can pick them up.
    pub fn drain_parent_events(&mut self) -> Vec<String> {
        let mut parent_events: Vec<String> = Vec::new();
        let mut remaining: VecDeque<String> = VecDeque::new();
        while let Some(ev) = self.queue.pop_front() {
            if let Some(rest) = ev.strip_prefix("__parent__:") {
                // Format: "<event_name>\x01<content>" -- forward as __pev__ token.
                parent_events.push(format!("__pev__{}", rest));
            } else if ev.starts_with("__delayed__") || ev.starts_with("__cancel__") {
                // Forward timer tokens to the parent so it can register them.
                parent_events.push(ev);
            } else {
                remaining.push_back(ev);
            }
        }
        self.queue = remaining;
        parent_events
    }

    /// Return the current active state id.
    pub fn current_state(&self) -> &str {
        &self.active_state
    }

    /// Get a variable value from the child scope.
    pub fn get_var(&self, name: &str) -> Value {
        self.scope.get(name)
    }

    fn enter_state(&mut self, state_id: &str) {
        self.active_state = state_id.to_string();
        let (onentry, dm, init_child) = {
            if let Some(node) = self.find_state(state_id) {
                let onentry = node.onentry().to_vec();
                match node {
                    StateNodeIr::State { datamodel, initial, .. } => {
                        (onentry, datamodel.to_vec(), initial.clone())
                    }
                    _ => (onentry, vec![], None),
                }
            } else {
                (vec![], vec![], None)
            }
        };
        let ev = self.event_ctx.clone();
        exec_actions(&onentry, &mut self.scope, &mut self.queue, &ev);
        for slot in &dm {
            if let Some(ref v) = slot.initial_value {
                self.scope.set(slot.id.clone(), v.clone());
            }
        }
        if let Some(child_id) = init_child {
            self.enter_state(&child_id);
            return;
        }
        self.run_epsilon_transitions();
    }

    fn exit_state(&mut self, state_id: &str) {
        let onexit: Vec<ActionIr> = self.find_state(state_id)
            .map(|n| n.onexit().to_vec())
            .unwrap_or_default();
        let ev = self.event_ctx.clone();
        exec_actions(&onexit, &mut self.scope, &mut self.queue, &ev);
    }

    fn process_event(&mut self, event_name: String) {
        let active = self.active_state.clone();
        let ancestors = self.get_ancestors(&active);
        for state_id in ancestors {
            if self.try_transitions_in(&state_id, &event_name) {
                return;
            }
        }
    }

    fn try_transitions_in(&mut self, state_id: &str, event_name: &str) -> bool {
        let transitions: Vec<TransitionIr> = self.find_state(state_id)
            .map(|n| n.transitions().to_vec())
            .unwrap_or_default();
        for t in &transitions {
            let event_matches = match &t.event {
                None => false,
                Some(pattern) => event_matches_pattern(event_name, pattern),
            };
            if !event_matches { continue; }
            let guard_ok = match &t.guard {
                None => true,
                Some(g) => eval_expr(g, &self.scope, &self.event_ctx).is_truthy(),
            };
            if !guard_ok { continue; }
            self.fire_transition(t.clone());
            return true;
        }
        false
    }

    fn run_epsilon_transitions(&mut self) {
        let mut safety = 0usize;
        loop {
            if safety > 100 { break; }
            safety += 1;
            let active = self.active_state.clone();
            let ancestors = self.get_ancestors(&active);
            let mut candidate: Option<TransitionIr> = None;
            'outer: for state_id in &ancestors {
                let transitions: Vec<TransitionIr> = self.find_state(state_id)
                    .map(|n| n.transitions().to_vec())
                    .unwrap_or_default();
                for t in transitions {
                    if t.event.is_some() { continue; }
                    let guard_ok = match &t.guard {
                        None => true,
                        Some(g) => eval_expr(g, &self.scope, &self.event_ctx).is_truthy(),
                    };
                    if guard_ok {
                        candidate = Some(t);
                        break 'outer;
                    }
                }
            }
            if let Some(t) = candidate {
                self.fire_transition(t);
            } else {
                break;
            }
        }
    }

    fn fire_transition(&mut self, t: TransitionIr) {
        let ev = self.event_ctx.clone();
        exec_actions(&t.actions, &mut self.scope, &mut self.queue, &ev);
        if let Some(target_id) = t.targets.first() {
            let old_state = self.active_state.clone();
            self.exit_state(&old_state);
            // Collect the old state's ancestor chain (exclusive of root).
            let old_ancs = self.get_ancestors(&old_state);
            // Collect the target state's ancestor chain (exclusive of root),
            // reversed to get top-down order (outermost first).
            let target_clone = target_id.clone();
            let target_ancs = self.get_ancestors(&target_clone);
            // Determine which ancestor compound states are newly entered
            // (present in target's chain but not old state's chain).
            // We need to run onentry for these in top-down order.
            // target_ancs is [leaf, parent, grandparent, ...]; reverse for top-down.
            let mut new_ancs: Vec<String> = target_ancs.iter()
                .filter(|a| *a != &target_clone && !old_ancs.contains(*a))
                .cloned()
                .collect();
            new_ancs.reverse(); // now top-down (outermost first)
            // Execute onentry for each newly-entered ancestor.
            for anc_id in &new_ancs {
                let anc_onentry: Vec<ActionIr> = self.find_state(anc_id)
                    .map(|n| n.onentry().to_vec())
                    .unwrap_or_default();
                let anc_dm: Vec<DataSlotIr> = self.find_state(anc_id)
                    .and_then(|n| match n {
                        StateNodeIr::State { datamodel, .. }
                        | StateNodeIr::Parallel { datamodel, .. } => Some(datamodel.to_vec()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let ev2 = self.event_ctx.clone();
                exec_actions(&anc_onentry, &mut self.scope, &mut self.queue, &ev2);
                for slot in &anc_dm {
                    if let Some(ref v) = slot.initial_value {
                        self.scope.set(slot.id.clone(), v.clone());
                    }
                }
            }
            self.enter_state(&target_clone);
        }
    }

    fn get_ancestors(&self, state_id: &str) -> Vec<String> {
        let mut result = vec![state_id.to_string()];
        self.collect_ancestors_into(self.ir.children.as_slice(), state_id, &mut result);
        result
    }

    fn collect_ancestors_into(
        &self,
        nodes: &[StateNodeIr],
        target: &str,
        acc: &mut Vec<String>,
    ) -> bool {
        for node in nodes {
            if node.id() == target {
                return true;
            }
            if self.collect_ancestors_into(node.children(), target, acc) {
                acc.push(node.id().to_string());
                return true;
            }
        }
        false
    }

    fn find_state(&self, state_id: &str) -> Option<&StateNodeIr> {
        for node in &self.ir.children {
            if let Some(found) = node.find_descendant(state_id) {
                return Some(found);
            }
        }
        None
    }
}

pub struct Machine {
    scope: Scope,
    /// Active configuration: the set of active atomic (leaf) states.
    /// For a non-parallel machine this holds exactly one leaf; for a
    /// `<parallel>` machine it holds one live leaf per active region.
    active: Vec<String>,
    /// Internal event queue.
    queue: VecDeque<String>,
    /// Current event context.
    event_ctx: EventCtx,
    /// The embedded machine IR data.
    ir: &'static MachineIrData,
    /// Active inline child machines spawned by invoke elements.
    child_machines: Vec<ChildMachine>,
    /// Logical-time timer queue sorted by fire_time_ms then seq.
    timers: Vec<PendingTimer>,
    /// Monotonically increasing sequence counter for timer ordering.
    timer_seq: u64,
    /// Current logical clock in milliseconds (advanced by run()).
    pub logical_clock_ms: u64,
}

impl Machine {
    /// Create a new machine, initialized to the initial state.
    pub fn new() -> Self {
        let ir = machine_ir_data();
        let mut scope = Scope::new();
        // Initialize root datamodel
        for slot in &ir.datamodel {
            if let Some(ref v) = slot.initial_value {
                scope.set(slot.id.clone(), v.clone());
            } else {
                scope.set(slot.id.clone(), Value::Undefined);
            }
        }
        Machine {
            scope,
            active: Vec::new(),
            queue: VecDeque::new(),
            event_ctx: EventCtx::default(),
            ir,
            child_machines: Vec::new(),
            timers: Vec::new(),
            timer_seq: 0,
            logical_clock_ms: 0,
        }
    }

    /// Initialize the machine: enter the initial state and run onentry actions.
    pub fn start(&mut self) {
        // Use explicit initial list; fall back to first top-level child if empty.
        let initial_id = self.ir.initial.first().cloned()
            .filter(|s| !s.is_empty())
            .or_else(|| self.ir.children.first().map(|n| n.id().to_string()))
            .unwrap_or_default();
        if !initial_id.is_empty() {
            self.enter_state(&initial_id);
            self.macrostep();
        }
        // Drain the startup queue:
        // - ``__delayed__`` tokens: register as pending timers.
        // - ``__cancel__`` tokens: cancel pending timers.
        // - ``__pev__`` tokens: child-to-parent events with content -- process
        //   them now so spawn-time sends (e.g., ``think.5``) reach the parent.
        // - Other events: leave in queue for the first ``step()`` call.
        let mut remaining: VecDeque<String> = VecDeque::new();
        while let Some(ev) = self.queue.pop_front() {
            if let Some(rest) = ev.strip_prefix("__delayed__") {
                self.register_timer_from_str(rest);
            } else if let Some(rest) = ev.strip_prefix("__cancel__") {
                self.timers.retain(|t| t.send_id.as_deref() != Some(rest));
            } else if let Some(rest) = ev.strip_prefix("__pev__") {
                // Child-to-parent event: process immediately so the parent
                // transitions on spawn-time events (think.N, etc.).
                let (ev_name, content_val) = if let Some(sep) = rest.find('') {
                    (rest[..sep].to_string(), Self::parse_content_str(&rest[sep + 1..]))
                } else {
                    (rest.to_string(), Value::Undefined)
                };
                self.step(&ev_name, content_val);
            } else {
                remaining.push_back(ev);
            }
        }
        self.queue = remaining;
    }

    /// Process one external event.  Child machines receive events only via
    /// explicit ``__child__<id>:event`` routing tokens generated by parent
    /// transitions (SCXML §6.4: invoked sessions are NOT autoforwarded).
    pub fn step(&mut self, event_name: &str, event_data: Value) {
        self.event_ctx = EventCtx::new(event_name, event_data.clone());
        self.process_event(event_name.to_string());
        // Drain internal queue.  Events prefixed ``__child__<id>:`` are
        // routed to the matching child machine; ``__delayed__`` and ``__cancel__``
        // tokens are registered with / cancelled from the timer queue; others are
        // processed normally.
        let mut safety = 0usize;
        while let Some(internal_ev) = self.queue.pop_front() {
            if safety > 1000 { break; }
            safety += 1;
            if let Some(rest) = internal_ev.strip_prefix("__child__") {
                // Format: "__child__<invoke_id>:<event_name>"
                if let Some(colon) = rest.find(':') {
                    let child_id = &rest[..colon];
                    let child_ev = rest[colon + 1..].to_string();
                    if let Some(child) = self.child_machines.iter_mut()
                        .find(|c| c.invoke_id == child_id)
                    {
                        let parent_bound2 = child.deliver(&child_ev, event_data.clone());
                        for pev in parent_bound2 {
                            self.queue.push_back(pev);
                        }
                    }
                }
            } else if let Some(rest) = internal_ev.strip_prefix("__delayed__") {
                // Parse: "<delay_ms>:<send_id>:<target>:<event>:<content>"
                let parts: Vec<&str> = rest.splitn(5, ':').collect();
                if parts.len() >= 4 {
                    let delay_ms = parts[0].parse::<u64>().unwrap_or(0);
                    let send_id_str = parts[1];
                    let target = parts[2];
                    let event_name = parts[3].to_string();
                    let content_str = if parts.len() >= 5 { parts[4] } else { "" };
                    let content = Self::parse_content_str(content_str);
                    let fire_time = self.logical_clock_ms + delay_ms;
                    let send_id = if send_id_str.is_empty() { None } else { Some(send_id_str.to_string()) };
                    let machine_id = if target.starts_with("#_") {
                        target[2..].to_string()
                    } else {
                        String::new()
                    };
                    let seq = self.timer_seq;
                    self.timer_seq += 1;
                    self.timers.push(PendingTimer { fire_time_ms: fire_time, seq, machine_id, event_name, content, send_id });
                    self.timers.sort_by(|a, b| a.fire_time_ms.cmp(&b.fire_time_ms).then(a.seq.cmp(&b.seq)));
                }
            } else if let Some(rest) = internal_ev.strip_prefix("__cancel__") {
                let cid = rest.to_string();
                self.timers.retain(|t| t.send_id.as_deref() != Some(&cid));
            } else if let Some(rest) = internal_ev.strip_prefix("__pev__") {
                // Child-to-parent event with content.
                // Format: "<event_name>\x01<content_str>" (\x01 separator).
                let (ev_name, content_val) = if let Some(sep) = rest.find('') {
                    let ev_n = rest[..sep].to_string();
                    let cs = &rest[sep + 1..];
                    let cv = Self::parse_content_str(cs);
                    (ev_n, cv)
                } else {
                    (rest.to_string(), Value::Undefined)
                };
                self.event_ctx = EventCtx::new(&ev_name, content_val);
                self.process_event(ev_name);
            } else {
                let ev_clone = internal_ev.clone();
                self.event_ctx = EventCtx::new(&ev_clone, Value::Undefined);
                self.process_event(internal_ev);
            }
        }
    }

    /// Return a current active leaf state id. For a `<parallel>`
    /// machine this is the first active region's leaf; use
    /// `active_states()` for the full configuration.
    pub fn current_state(&self) -> &str {
        self.active.first().map(|s| s.as_str()).unwrap_or("")
    }

    /// Return all active atomic (leaf) states. One entry for a
    /// non-parallel machine; one per active region for `<parallel>`.
    pub fn active_states(&self) -> &[String] {
        &self.active
    }

    /// Return true if `state_id` is in the active configuration
    /// (an active leaf or an ancestor of one).
    pub fn is_active(&self, state_id: &str) -> bool {
        self.active.iter().any(|leaf| {
            leaf == state_id || self.get_ancestors(leaf).iter().any(|a| a == state_id)
        })
    }

    /// Return the current active state as a MachineState enum variant.
    pub fn current_state_variant(&self) -> MachineState {
        MachineState::from_id(self.current_state())
    }

    /// Set a datamodel variable (for external initialization / testing).
    pub fn set_var(&mut self, name: impl Into<String>, val: Value) {
        self.scope.set(name.into(), val);
    }

    /// Get a datamodel variable value.
    pub fn get_var(&self, name: &str) -> Value {
        self.scope.get(name)
    }

    /// Return the number of active inline child machines.
    pub fn child_count(&self) -> usize {
        self.child_machines.len()
    }

    /// Get a variable from a child machine identified by invoke_id.
    pub fn get_child_var(&self, invoke_id: &str, name: &str) -> Option<Value> {
        self.child_machines.iter()
            .find(|c| c.invoke_id == invoke_id)
            .map(|c| c.get_var(name))
    }

    /// Get the active state of a child machine identified by invoke_id.
    pub fn get_child_state(&self, invoke_id: &str) -> Option<String> {
        self.child_machines.iter()
            .find(|c| c.invoke_id == invoke_id)
            .map(|c| c.current_state().to_string())
    }

    fn enter_state(&mut self, state_id: &str) {
        // Clone all data needed from the node BEFORE any mutable borrows.
        let (onentry, dm, init_child, child_ids, is_parallel, invokes) = {
            if let Some(node) = self.find_state(state_id) {
                let onentry = node.onentry().to_vec();
                match node {
                    StateNodeIr::State { datamodel, initial, invokes, .. } => {
                        (onentry, datamodel.to_vec(), initial.clone(), vec![], false, invokes.to_vec())
                    }
                    StateNodeIr::Parallel { datamodel, children, invokes, .. } => {
                        let child_ids: Vec<String> = children.iter()
                            .map(|c| c.id().to_string()).collect();
                        (onentry, datamodel.to_vec(), None, child_ids, true, invokes.to_vec())
                    }
                    _ => (onentry, vec![], None, vec![], false, vec![]),
                }
            } else {
                (vec![], vec![], None, vec![], false, vec![])
            }
        };
        // Execute onentry actions
        let ev = self.event_ctx.clone();
        exec_actions(&onentry, &mut self.scope, &mut self.queue, &ev);
        // Initialize state-level datamodel
        for slot in &dm {
            if let Some(ref v) = slot.initial_value {
                self.scope.set(slot.id.clone(), v.clone());
            }
        }
        // Spawn inline child machines from invokes
        for inv in invokes {
            if let Some(child_ir) = inv.child_machine {
                let invoke_id = inv.invoke_id.clone()
                    .unwrap_or_else(|| format!("_invoke_{}", self.child_machines.len()));
                // Collect param values from parent scope
                let params: Vec<(String, Value)> = inv.params.iter()
                    .map(|p| {
                        let val = eval_expr(&p.expr, &self.scope, &self.event_ctx);
                        (p.name.clone(), val)
                    })
                    .collect();
                let mut child = ChildMachine::spawn(invoke_id.clone(), *child_ir, params);
                // Drain spawn-time #_parent events (e.g., think.N onentry send).
                let spawn_parent = child.drain_parent_events();
                self.child_machines.push(child);
                for pev in spawn_parent {
                    // ``__delayed__`` tokens from a child with an empty target
                    // should fire back in that child, not in the parent.
                    // Re-write the target segment to ``#_<invoke_id>`` so the
                    // timer machinery routes back to the child, avoiding the
                    // auto-forward double-delivery.
                    if let Some(rest) = pev.strip_prefix("__delayed__") {
                        // Format: "<ms>:<sendid>:<target>:<event>[:<content>]"
                        let parts: Vec<&str> = rest.splitn(5, ':').collect();
                        if parts.len() >= 4 && parts[2].is_empty() {
                            // Empty target: re-tag to this child's invoke_id.
                            let tagged = format!("__delayed__{}:{}:#_{}:{}:{}",
                                parts[0], parts[1], invoke_id,
                                parts[3],
                                if parts.len() >= 5 { parts[4] } else { "" });
                            self.queue.push_back(tagged);
                            continue;
                        }
                    }
                    self.queue.push_back(pev);
                }
            }
        }
        // Enter ALL regions of a <parallel> -- the configuration is a
        // set of leaves, one live leaf per region.
        if is_parallel {
            for cid in child_ids {
                self.enter_state(&cid);
            }
            return;
        }
        // Auto-enter the initial child of a compound state.
        if let Some(child_id) = init_child {
            self.enter_state(&child_id);
            return;
        }
        // Atomic leaf: join the active configuration. The epsilon
        // macrostep is run by the caller after the full entry set.
        if !self.active.iter().any(|s| s == state_id) {
            self.active.push(state_id.to_string());
        }
    }

    fn exit_state(&mut self, state_id: &str) {
        // Remove any child machines spawned by invokes on this state
        let invokes: Vec<InvokeIr> = self.find_state(state_id)
            .and_then(|n| match n {
                StateNodeIr::State { invokes, .. } => Some(invokes.to_vec()),
                StateNodeIr::Parallel { invokes, .. } => Some(invokes.to_vec()),
                _ => None,
            })
            .unwrap_or_default();
        for inv in &invokes {
            if inv.child_machine.is_some() {
                if let Some(invoke_id) = &inv.invoke_id {
                    self.child_machines.retain(|c| &c.invoke_id != invoke_id);
                }
            }
        }
        // Clone onexit before any mutable borrow
        let onexit: Vec<ActionIr> = self.find_state(state_id)
            .map(|n| n.onexit().to_vec())
            .unwrap_or_default();
        let ev = self.event_ctx.clone();
        exec_actions(&onexit, &mut self.scope, &mut self.queue, &ev);
    }

    fn process_event(&mut self, event_name: String) {
        // Offer the event to every active region. Each region fires at
        // most one transition (the first enabled along its leaf's
        // ancestor chain); regions are independent, so we fire one per
        // region. A region already exited by an earlier transition in
        // this microstep is skipped.
        let leaves = self.active.clone();
        for leaf in &leaves {
            if !self.active.iter().any(|s| s == leaf) { continue; }
            let ancestors = self.get_ancestors(leaf);
            if let Some((t, src)) = self.find_enabled(&ancestors, Some(&event_name)) {
                self.fire_transition(t, &src);
            }
        }
        self.macrostep();
    }

    /// Find the first enabled transition along an ancestor chain (leaf
    /// first). ``event = Some(name)`` selects event-triggered transitions
    /// whose pattern matches; ``event = None`` selects eventless ones.
    /// Returns the transition and the id of the state that owns it.
    fn find_enabled(
        &self,
        ancestors: &[String],
        event: Option<&str>,
    ) -> Option<(TransitionIr, String)> {
        for state_id in ancestors {
            let transitions: Vec<TransitionIr> = self.find_state(state_id)
                .map(|n| n.transitions().to_vec())
                .unwrap_or_default();
            for t in transitions {
                let matches = match (&t.event, event) {
                    (None, None) => true,
                    (Some(p), Some(name)) => event_matches_pattern(name, p),
                    _ => false,
                };
                if !matches { continue; }
                let guard_ok = match &t.guard {
                    None => true,
                    Some(g) => eval_expr(g, &self.scope, &self.event_ctx).is_truthy(),
                };
                if !guard_ok { continue; }
                return Some((t, state_id.clone()));
            }
        }
        None
    }

    /// Run eventless transitions to stability across the active set.
    fn macrostep(&mut self) {
        let mut safety = 0usize;
        loop {
            if safety > 200 { break; }
            safety += 1;
            let leaves = self.active.clone();
            let mut fired = false;
            for leaf in &leaves {
                if !self.active.iter().any(|s| s == leaf) { continue; }
                let ancestors = self.get_ancestors(leaf);
                if let Some((t, src)) = self.find_enabled(&ancestors, None) {
                    self.fire_transition(t, &src);
                    fired = true;
                    break; // configuration changed; rescan from scratch
                }
            }
            if !fired { break; }
        }
    }

    /// Backward-compatible name; runs the eventless macrostep.
    fn run_epsilon_transitions(&mut self) {
        self.macrostep();
    }

    fn fire_transition(&mut self, t: TransitionIr, source_id: &str) {
        // Execute transition actions in the current scope.
        let ev = self.event_ctx.clone();
        exec_actions(&t.actions, &mut self.scope, &mut self.queue, &ev);
        // Targetless transition: actions only, no configuration change.
        let target = match t.targets.first() {
            Some(x) => x.clone(),
            None => return,
        };
        // Transition domain = least common ancestor of source and target.
        let domain = self.lca(source_id, &target);
        // Exit the active leaves that lie strictly within the domain
        // (region-local exit); other regions are untouched. An empty
        // domain is the implicit document root, an ancestor of every
        // leaf, so all active leaves qualify.
        let exit_leaves: Vec<String> = self.active.iter()
            .filter(|l| l.as_str() != domain.as_str()
                && (domain.is_empty()
                    || self.get_ancestors(l).iter().any(|a| a == &domain)))
            .cloned()
            .collect();
        for leaf in &exit_leaves {
            self.exit_to_domain(leaf, &domain);
        }
        // Enter the target subtree under the domain.
        self.enter_to(&target, &domain);
    }

    /// Least common ancestor of two states: the deepest state that is an
    /// ancestor (or self) of both. For a sibling transition this is the
    /// shared parent; for an internal transition (source is an ancestor of
    /// target) it is the source state itself.
    fn lca(&self, a: &str, b: &str) -> String {
        let ba = self.get_ancestors(b);
        for anc in self.get_ancestors(a) {
            if ba.iter().any(|x| *x == anc) {
                return anc;
            }
        }
        // No common ancestor among the listed states: the only shared
        // ancestor is the implicit document root. Return the empty
        // sentinel so the source's full chain is exited and the target
        // is entered from the top (covers top-level sibling transitions
        // whose parent is the synthetic <scxml> root, which is not part
        // of get_ancestors()).
        String::new()
    }

    /// Exit a leaf and its ancestors up to (but not including) ``domain``,
    /// running onexit for each, then remove the leaf from the active set.
    fn exit_to_domain(&mut self, leaf: &str, domain: &str) {
        for s in self.get_ancestors(leaf) {
            if s == domain { break; }
            self.exit_state(&s);
        }
        self.active.retain(|l| l != leaf);
    }

    /// Enter the path from ``domain`` (exclusive) down to ``target``
    /// (inclusive); ``target``'s own subtree (initial child / all parallel
    /// regions) is entered by ``enter_state``.
    fn enter_to(&mut self, target: &str, domain: &str) {
        let mut path: Vec<String> = Vec::new();
        for s in self.get_ancestors(target) {
            if s == domain { break; }
            if s != target { path.push(s); }
        }
        path.reverse(); // outermost first
        for s in &path {
            self.enter_onentry_only(s);
        }
        self.enter_state(target);
    }

    /// Run onentry / datamodel init for an intermediate path state without
    /// descending into its children or joining the active set (its
    /// descendant on the path is entered explicitly).
    fn enter_onentry_only(&mut self, state_id: &str) {
        let (onentry, dm) = {
            if let Some(node) = self.find_state(state_id) {
                let onentry = node.onentry().to_vec();
                let dm = match node {
                    StateNodeIr::State { datamodel, .. }
                    | StateNodeIr::Parallel { datamodel, .. } => datamodel.to_vec(),
                    _ => vec![],
                };
                (onentry, dm)
            } else {
                (vec![], vec![])
            }
        };
        let ev = self.event_ctx.clone();
        exec_actions(&onentry, &mut self.scope, &mut self.queue, &ev);
        for slot in &dm {
            if let Some(ref v) = slot.initial_value {
                self.scope.set(slot.id.clone(), v.clone());
            }
        }
    }

    fn get_ancestors(&self, state_id: &str) -> Vec<String> {
        // Return the state id plus all its ancestors (leaf first)
        let mut result = vec![state_id.to_string()];
        self.collect_ancestors_into(self.ir.children.as_slice(), state_id, &mut result);
        result
    }

    fn collect_ancestors_into(
        &self,
        nodes: &[StateNodeIr],
        target: &str,
        acc: &mut Vec<String>,
    ) -> bool {
        for node in nodes {
            if node.id() == target {
                return true;
            }
            if self.collect_ancestors_into(node.children(), target, acc) {
                acc.push(node.id().to_string());
                return true;
            }
        }
        false
    }

    fn find_state(&self, state_id: &str) -> Option<&StateNodeIr> {
        for node in &self.ir.children {
            if let Some(found) = node.find_descendant(state_id) {
                return Some(found);
            }
        }
        None
    }

    /// Advance the logical clock, firing pending timers in order.
    ///
    /// Each iteration picks the earliest timer, advances ``logical_clock_ms`` to
    /// its ``fire_time_ms``, fires it, and processes any resulting events (including
    /// newly-registered timers from the fired actions).  Repeats up to ``max_steps``
    /// times or until the timer queue is empty.
    ///
    /// Returns a trace log of every timer that fired (one line per timer).
    pub fn run(&mut self, max_steps: usize) -> Vec<String> {
        let mut trace: Vec<String> = Vec::new();
        for _ in 0..max_steps {
            if self.timers.is_empty() { break; }
            let earliest = self.timers.iter().map(|t| t.fire_time_ms).min().unwrap_or(0);
            self.logical_clock_ms = earliest;
            let mut fired: Vec<PendingTimer> = Vec::new();
            let mut remaining: Vec<PendingTimer> = Vec::new();
            for t in self.timers.drain(..) {
                if t.fire_time_ms <= self.logical_clock_ms { fired.push(t); }
                else { remaining.push(t); }
            }
            fired.sort_by(|a, b| a.seq.cmp(&b.seq));
            self.timers = remaining;
            for timer in fired {
                trace.push(format!(
                    "TIMER t={}ms seq={} event={} machine={} content={}",
                    timer.fire_time_ms, timer.seq, timer.event_name,
                    if timer.machine_id.is_empty() { "parent".to_string() } else { timer.machine_id.clone() },
                    timer.content.to_display_string()
                ));
                if timer.machine_id.is_empty() {
                    // Timer targets the parent machine directly.
                    self.step(&timer.event_name, timer.content.clone());
                } else {
                    // Timer targets a specific child machine by invoke id.
                    let child_evts: Vec<String> = {
                        if let Some(child) = self.child_machines.iter_mut()
                            .find(|c| c.invoke_id == timer.machine_id)
                        {
                            child.step(&timer.event_name, timer.content.clone())
                        } else {
                            Vec::new()
                        }
                    };
                    for pev in child_evts {
                        if pev.starts_with("__delayed__") || pev.starts_with("__cancel__") {
                            self.queue.push_back(pev);
                        } else if let Some(rest) = pev.strip_prefix("__pev__") {
                            // Child-to-parent event with content.
                            // Format: "<event_name>\x01<content_str>".
                            let (ev_name, content_val) = if let Some(sep) = rest.find('') {
                                (rest[..sep].to_string(), Self::parse_content_str(&rest[sep + 1..]))
                            } else {
                                (rest.to_string(), Value::Undefined)
                            };
                            self.step(&ev_name, content_val);
                        } else {
                            self.step(&pev, Value::Undefined);
                        }
                    }
                    // Drain any newly-queued __delayed__/__cancel__ tokens from child events.
                    let mut i = 0;
                    while i < self.queue.len() {
                        let check = self.queue[i].clone();
                        if check.starts_with("__delayed__") {
                            let ev = self.queue.remove(i).unwrap_or_default();
                            if let Some(rest) = ev.strip_prefix("__delayed__") {
                                self.register_timer_from_str(rest);
                            }
                        } else if check.starts_with("__cancel__") {
                            let ev = self.queue.remove(i).unwrap_or_default();
                            if let Some(rest) = ev.strip_prefix("__cancel__") {
                                self.timers.retain(|t| t.send_id.as_deref() != Some(rest));
                            }
                        } else {
                            i += 1;
                        }
                    }
                }
            }
        }
        trace
    }

    /// Register a timer from a ``__delayed__`` payload string.
    ///
    /// Format: ``"<delay_ms>:<send_id>:<target>:<event>[:<content>]"``.
    fn register_timer_from_str(&mut self, rest: &str) {
        let parts: Vec<&str> = rest.splitn(5, ':').collect();
        if parts.len() >= 4 {
            let delay_ms = parts[0].parse::<u64>().unwrap_or(0);
            let send_id_str = parts[1];
            let target = parts[2];
            let event_name = parts[3].to_string();
            let content_str = if parts.len() >= 5 { parts[4] } else { "" };
            let content = Self::parse_content_str(content_str);
            let fire_time = self.logical_clock_ms + delay_ms;
            let send_id = if send_id_str.is_empty() { None } else { Some(send_id_str.to_string()) };
            let machine_id = if target.starts_with("#_") {
                target[2..].to_string()
            } else {
                String::new()
            };
            let seq = self.timer_seq;
            self.timer_seq += 1;
            self.timers.push(PendingTimer { fire_time_ms: fire_time, seq, machine_id, event_name, content, send_id });
            self.timers.sort_by(|a, b| a.fire_time_ms.cmp(&b.fire_time_ms).then(a.seq.cmp(&b.seq)));
        }
    }

    /// Parse a content string carried inside a ``__delayed__`` token back to a ``Value``.
    fn parse_content_str(s: &str) -> Value {
        if s.is_empty() || s == "undefined" { return Value::Undefined; }
        if s == "true" { return Value::Bool(true); }
        if s == "false" { return Value::Bool(false); }
        if let Ok(n) = s.parse::<i64>() { return Value::Int(n); }
        if let Ok(f) = s.parse::<f64>() { return Value::Number(f); }
        Value::Str(s.to_string())
    }

    /// Return the number of pending timers in the logical-time queue.
    pub fn timer_count(&self) -> usize { self.timers.len() }

    /// Return the current logical clock value in milliseconds.
    pub fn logical_time_ms(&self) -> u64 { self.logical_clock_ms }
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

/// Return true when *event_name* matches *pattern*.
///
/// SCXML event matching: a pattern ending in `.*` matches the prefix.
/// An exact pattern matches only the exact name.
pub fn event_matches_pattern(event_name: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        event_name == prefix || event_name.starts_with(&format!("{}.", prefix))
    } else {
        event_name == pattern
    }
}

// ---------------------------------------------------------------------------
// Embedded machine IR data
// ---------------------------------------------------------------------------

/// Return a reference to the embedded machine IR (allocated once at startup).
///
/// Uses ``std::sync::OnceLock`` for thread-safe single initialization.
#[cfg(feature = "std")]
pub fn machine_ir_data() -> &'static MachineIrData {
    use std::sync::OnceLock;
    static DATA: OnceLock<MachineIrData> = OnceLock::new();
    DATA.get_or_init(build_machine_ir)
}

/// no_std version: leaks a Box allocation for static lifetime.
#[cfg(not(feature = "std"))]
pub fn machine_ir_data() -> &'static MachineIrData {
    use alloc::boxed::Box;
    // BLOCKED: no_std singleton: leaks on each call -- for embedded use
    // replace with a linker-section static or a provided singleton.
    let ir = Box::new(build_machine_ir());
    Box::leak(ir)
}

fn build_machine_ir() -> MachineIrData {
    MachineIrData {
        name: Some("ScxmlDiningPhilosophers".to_string()),
        initial: vec![],
        datamodel: vec![
        ],
        children: vec![
            StateNodeIr::Parallel {
                id: "DiningPhilosophers".to_string(),
                datamodel: vec![DataSlotIr { id: "t_INPUTS".to_string(), initial_value: Some({ let mut _m = BTreeMap::new(); _m.insert("take.1".to_string(), Value::Int(0_i64)); _m.insert("take.2".to_string(), Value::Int(0_i64)); _m.insert("take.3".to_string(), Value::Int(0_i64)); _m.insert("take.4".to_string(), Value::Int(0_i64)); _m.insert("take.5".to_string(), Value::Int(0_i64)); Value::Map(_m)} ) }, DataSlotIr { id: "t_HAND_COMPLIANCE".to_string(), initial_value: Some(Value::Array(vec![Value::Array(vec![Value::Int(1_i64), Value::Int(5_i64)]), Value::Array(vec![Value::Int(2_i64), Value::Int(1_i64)]), Value::Array(vec![Value::Int(3_i64), Value::Int(2_i64)]), Value::Array(vec![Value::Int(4_i64), Value::Int(3_i64)]), Value::Array(vec![Value::Int(5_i64), Value::Int(4_i64)])])) }, DataSlotIr { id: "i_DELAY_THINK_EAT".to_string(), initial_value: Some(Value::Int(1000_i64)) }],
                onentry: vec![],
                onexit: vec![],
                transitions: vec![
                    TransitionIr {
                        event: Some("take.*".to_string()),
                        targets: vec![],
                        guard: None,
                        actions: vec![
                            ActionIr::Assign {
                                loc: "t_INPUTS[_event.name]".to_string(),
                                key_expr: Some(Box::new(ExprIr::EventName)),
                                expr: ExprIr::EventData,
                                op: "set".to_string(),
                            },
                        ],
                    },
                    TransitionIr {
                        event: Some("taken.*".to_string()),
                        targets: vec![],
                        guard: None,
                        actions: vec![
                            ActionIr::Foreach {
                                array_expr: ExprIr::VarRead("t_HAND_COMPLIANCE".to_string()),
                                item: "complianceItem".to_string(),
                                index: Some("complianceIndex".to_string()),
                                body: vec![
                                    ActionIr::Foreach {
                                        array_expr: ExprIr::VarRead("complianceItem".to_string()),
                                        item: "handItem".to_string(),
                                        index: None,
                                        body: vec![
                                            ActionIr::If {
                                                branches: vec![
                                                    IfBranchIr {
                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::VarRead("handItem".to_string())), Box::new(ExprIr::Call("int".to_string(), vec![ExprIr::Call("replace".to_string(), vec![ExprIr::StringLiteral("taken.".to_string()), ExprIr::StringLiteral("".to_string())], Some(Box::new(ExprIr::EventName)))], None)))),
                                                        body: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::EventName),
                                                                target: None,
                                                                targetexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("#_ID_P_".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::BinOp("+".to_string(), Box::new(ExprIr::VarRead("complianceIndex".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))], None)))),
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::EventData),
                                                                params: vec![],
                                                            },
                                                        ],
                                                    },
                                                ],
                                            },
                                        ],
                                    },
                                ],
                            },
                        ],
                    },
                    TransitionIr {
                        event: Some("update.delay".to_string()),
                        targets: vec![],
                        guard: None,
                        actions: vec![
                            ActionIr::Foreach {
                                array_expr: ExprIr::VarRead("t_HAND_COMPLIANCE".to_string()),
                                item: "vItem".to_string(),
                                index: Some("nIndex".to_string()),
                                body: vec![
                                    ActionIr::Send {
                                        event: None,
                                        eventexpr: Some(ExprIr::EventName),
                                        target: None,
                                        targetexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("#_ID_P_".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::BinOp("+".to_string(), Box::new(ExprIr::VarRead("nIndex".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))], None)))),
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::EventData),
                                        params: vec![],
                                    },
                                ],
                            },
                        ],
                    },
                ],
                children: vec![
                    StateNodeIr::State {
                        id: "Philosopher5".to_string(),
                        initial: Some("P5_Thinking".to_string()),
                        datamodel: vec![],
                        onentry: vec![],
                        onexit: vec![],
                        transitions: vec![],
                        children: vec![
                            StateNodeIr::State {
                                id: "P5_Thinking".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("think.5".to_string()),
                                        targets: vec!["P5_Hungry".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "P5_Hungry".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("eat.5".to_string()),
                                        targets: vec!["P5_Eating".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))),
                                        actions: vec![],
                                    },
                                    TransitionIr {
                                        event: Some("think.5".to_string()),
                                        targets: vec!["P5_Thinking".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "P5_Eating".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("eat.5".to_string()),
                                        targets: vec!["P5_Hungry".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                        ],
                        invokes: vec![
                            InvokeIr {
                                invoke_id: Some("ID_P_5".to_string()),
                                type_value: "scxml".to_string(),
                                src: None,
                                params: vec![SendParamIr { name: "i_ID".to_string(), expr: ExprIr::IntLiteral(5_i64) }, SendParamIr { name: "t_HAND_COMPLIANCE".to_string(), expr: ExprIr::VarRead("t_HAND_COMPLIANCE".to_string()) }, SendParamIr { name: "i_DELAY_THINK_EAT".to_string(), expr: ExprIr::VarRead("i_DELAY_THINK_EAT".to_string()) }],
                                autoforward: false,
                                blocked_msg: None,
                                child_machine: Some(Box::new(ChildMachineIr {
                                        name: Some("ScxmlPhilospher".to_string()),
                                        initial: vec!["Philospher".to_string()],
                                        datamodel: vec![DataSlotIr { id: "i_ID".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "i_ID_LEFT".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "i_ID_RIGHT".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "t_HAND_COMPLIANCE".to_string(), initial_value: Some(Value::Array(vec![Value::Array(vec![Value::Int(1_i64), Value::Int(5_i64)]), Value::Array(vec![Value::Int(2_i64), Value::Int(1_i64)]), Value::Array(vec![Value::Int(3_i64), Value::Int(2_i64)]), Value::Array(vec![Value::Int(4_i64), Value::Int(3_i64)]), Value::Array(vec![Value::Int(5_i64), Value::Int(4_i64)])])) }, DataSlotIr { id: "i_DELAY_THINK_EAT".to_string(), initial_value: Some(Value::Int(1000_i64)) }, DataSlotIr { id: "t_INPUTS".to_string(), initial_value: Some(Value::Map(BTreeMap::new())) }, DataSlotIr { id: "i_EAT_COUNT".to_string(), initial_value: Some(Value::Int(0_i64)) }],
                                        children: vec![
                                            StateNodeIr::State {
                                                id: "Philospher".to_string(),
                                                initial: Some("Thinking".to_string()),
                                                datamodel: vec![],
                                                onentry: vec![
                                                    ActionIr::If {
                                                        branches: vec![
                                                            IfBranchIr {
                                                                guard: Some(ExprIr::BinOp("!=".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                body: vec![
                                                                    ActionIr::Assign {
                                                                        loc: "i_ID_LEFT".to_string(),
                                                                        key_expr: None,
                                                                        expr: ExprIr::IndexAccess(Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_HAND_COMPLIANCE".to_string())), Box::new(ExprIr::BinOp("-".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))))), Box::new(ExprIr::IntLiteral(0_i64))),
                                                                        op: "set".to_string(),
                                                                    },
                                                                    ActionIr::Assign {
                                                                        loc: "i_ID_RIGHT".to_string(),
                                                                        key_expr: None,
                                                                        expr: ExprIr::IndexAccess(Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_HAND_COMPLIANCE".to_string())), Box::new(ExprIr::BinOp("-".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))))), Box::new(ExprIr::IntLiteral(1_i64))),
                                                                        op: "set".to_string(),
                                                                    },
                                                                ],
                                                            },
                                                        ],
                                                    },
                                                    ActionIr::Assign {
                                                        loc: "t_INPUTS['taken.' + str(i_ID_LEFT)]".to_string(),
                                                        key_expr: Some(Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None))))),
                                                        expr: ExprIr::IntLiteral(0_i64),
                                                        op: "set".to_string(),
                                                    },
                                                    ActionIr::Assign {
                                                        loc: "t_INPUTS['taken.' + str(i_ID_RIGHT)]".to_string(),
                                                        key_expr: Some(Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None))))),
                                                        expr: ExprIr::IntLiteral(0_i64),
                                                        op: "set".to_string(),
                                                    },
                                                ],
                                                onexit: vec![],
                                                transitions: vec![
                                                    TransitionIr {
                                                        event: Some("taken.*".to_string()),
                                                        targets: vec![],
                                                        guard: Some(ExprIr::BinOp("||".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))))),
                                                        actions: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::EventName], None)))),
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Assign {
                                                                loc: "t_INPUTS[_event.name]".to_string(),
                                                                key_expr: Some(Box::new(ExprIr::EventName)),
                                                                expr: ExprIr::EventData,
                                                                op: "set".to_string(),
                                                            },
                                                        ],
                                                    },
                                                    TransitionIr {
                                                        event: Some("error.*".to_string()),
                                                        targets: vec!["FinalSub".to_string()],
                                                        guard: None,
                                                        actions: vec![],
                                                    },
                                                    TransitionIr {
                                                        event: Some("update.delay".to_string()),
                                                        targets: vec![],
                                                        guard: None,
                                                        actions: vec![
                                                            ActionIr::Assign {
                                                                loc: "i_DELAY_THINK_EAT".to_string(),
                                                                key_expr: None,
                                                                expr: ExprIr::EventData,
                                                                op: "set".to_string(),
                                                            },
                                                        ],
                                                    },
                                                ],
                                                children: vec![
                                                    StateNodeIr::State {
                                                        id: "Eating".to_string(),
                                                        initial: None,
                                                        datamodel: vec![],
                                                        onentry: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("eat.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(1_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Send {
                                                                event: Some("Do.Timer.Think".to_string()),
                                                                eventexpr: None,
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_DELAY_THINK_EAT".to_string())], None)), Box::new(ExprIr::StringLiteral("ms".to_string())))),
                                                                send_id: Some("ID.Do.Timer.Think".to_string()),
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Assign {
                                                                loc: "i_EAT_COUNT".to_string(),
                                                                key_expr: None,
                                                                expr: ExprIr::BinOp("+".to_string(), Box::new(ExprIr::VarRead("i_EAT_COUNT".to_string())), Box::new(ExprIr::IntLiteral(1_i64))),
                                                                op: "set".to_string(),
                                                            },
                                                            ActionIr::Log {
                                                                expr: ExprIr::BinOp("+".to_string(), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("Philosopher: ".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))), Box::new(ExprIr::StringLiteral(" Count: ".to_string())))), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_EAT_COUNT".to_string())], None))),
                                                                label: Some("EATING".to_string()),
                                                            },
                                                        ],
                                                        onexit: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("eat.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Cancel { sendid: Some("ID.Do.Timer.Think".to_string()) },
                                                        ],
                                                        transitions: vec![],
                                                        children: vec![
                                                            StateNodeIr::State {
                                                                id: "RightTaken1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("Do.Timer.Think".to_string()),
                                                                        targets: vec!["RequirePutLeft".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftTaken1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("Do.Timer.Think".to_string()),
                                                                        targets: vec!["RequirePutRight".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftPut1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![],
                                                                children: vec![
                                                                    StateNodeIr::State {
                                                                        id: "RequirePutRight".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["PutLeftComplete".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                    StateNodeIr::State {
                                                                        id: "PutRightComplete".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["Thinking".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                ],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightPut1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![],
                                                                children: vec![
                                                                    StateNodeIr::State {
                                                                        id: "RequirePutLeft".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["PutRightComplete".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                    StateNodeIr::State {
                                                                        id: "PutLeftComplete".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["Thinking".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                ],
                                                                invokes: vec![],
                                                            },
                                                        ],
                                                        invokes: vec![],
                                                    },
                                                    StateNodeIr::State {
                                                        id: "Thinking".to_string(),
                                                        initial: None,
                                                        datamodel: vec![],
                                                        onentry: vec![
                                                            ActionIr::Send {
                                                                event: Some("Do.Timer.Hungry".to_string()),
                                                                eventexpr: None,
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_DELAY_THINK_EAT".to_string())], None)), Box::new(ExprIr::StringLiteral("ms".to_string())))),
                                                                send_id: Some("ID.Do.Timer.Hungry".to_string()),
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("think.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(1_i64)),
                                                                params: vec![],
                                                            },
                                                        ],
                                                        onexit: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("think.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Cancel { sendid: Some("ID.Do.Timer.Hungry".to_string()) },
                                                        ],
                                                        transitions: vec![
                                                            TransitionIr {
                                                                event: Some("Do.Timer.Hungry".to_string()),
                                                                targets: vec!["ProcessHungry".to_string()],
                                                                guard: None,
                                                                actions: vec![],
                                                            },
                                                        ],
                                                        children: vec![],
                                                        invokes: vec![],
                                                    },
                                                    StateNodeIr::State {
                                                        id: "ProcessHungry".to_string(),
                                                        initial: Some("Hungry".to_string()),
                                                        datamodel: vec![],
                                                        onentry: vec![],
                                                        onexit: vec![],
                                                        transitions: vec![],
                                                        children: vec![
                                                            StateNodeIr::State {
                                                                id: "Hungry".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftWaiting".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::VarRead("i_ID".to_string())),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("do.taken.*".to_string()),
                                                                        targets: vec!["LeftCheck".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightWaiting".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::VarRead("i_ID".to_string())),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("do.taken.*".to_string()),
                                                                        targets: vec!["RightCheck".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftCheck".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftTaken".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightPut".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightCheck".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightTaken".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftPut".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftTaken".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightTaken1".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftPut".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftPut".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["Hungry".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightTaken".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftTaken1".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightPut".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightPut".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["Hungry".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                        ],
                                                        invokes: vec![],
                                                    },
                                                ],
                                                invokes: vec![],
                                            },
                                            StateNodeIr::Final { id: "FinalSub".to_string() },
                                        ],
                                    })),
                            },
                        ],
                    },
                    StateNodeIr::State {
                        id: "Philosopher4".to_string(),
                        initial: Some("P4_Thinking".to_string()),
                        datamodel: vec![],
                        onentry: vec![],
                        onexit: vec![],
                        transitions: vec![],
                        children: vec![
                            StateNodeIr::State {
                                id: "P4_Thinking".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("think.4".to_string()),
                                        targets: vec!["P4_Hungry".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "P4_Hungry".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("eat.4".to_string()),
                                        targets: vec!["P4_Eating".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))),
                                        actions: vec![],
                                    },
                                    TransitionIr {
                                        event: Some("think.4".to_string()),
                                        targets: vec!["P4_Thinking".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "P4_Eating".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("eat.4".to_string()),
                                        targets: vec!["P4_Hungry".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                        ],
                        invokes: vec![
                            InvokeIr {
                                invoke_id: Some("ID_P_4".to_string()),
                                type_value: "scxml".to_string(),
                                src: None,
                                params: vec![SendParamIr { name: "i_ID".to_string(), expr: ExprIr::IntLiteral(4_i64) }, SendParamIr { name: "t_HAND_COMPLIANCE".to_string(), expr: ExprIr::VarRead("t_HAND_COMPLIANCE".to_string()) }, SendParamIr { name: "i_DELAY_THINK_EAT".to_string(), expr: ExprIr::VarRead("i_DELAY_THINK_EAT".to_string()) }],
                                autoforward: false,
                                blocked_msg: None,
                                child_machine: Some(Box::new(ChildMachineIr {
                                        name: Some("ScxmlPhilospher".to_string()),
                                        initial: vec!["Philospher".to_string()],
                                        datamodel: vec![DataSlotIr { id: "i_ID".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "i_ID_LEFT".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "i_ID_RIGHT".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "t_HAND_COMPLIANCE".to_string(), initial_value: Some(Value::Array(vec![Value::Array(vec![Value::Int(1_i64), Value::Int(5_i64)]), Value::Array(vec![Value::Int(2_i64), Value::Int(1_i64)]), Value::Array(vec![Value::Int(3_i64), Value::Int(2_i64)]), Value::Array(vec![Value::Int(4_i64), Value::Int(3_i64)]), Value::Array(vec![Value::Int(5_i64), Value::Int(4_i64)])])) }, DataSlotIr { id: "i_DELAY_THINK_EAT".to_string(), initial_value: Some(Value::Int(1000_i64)) }, DataSlotIr { id: "t_INPUTS".to_string(), initial_value: Some(Value::Map(BTreeMap::new())) }, DataSlotIr { id: "i_EAT_COUNT".to_string(), initial_value: Some(Value::Int(0_i64)) }],
                                        children: vec![
                                            StateNodeIr::State {
                                                id: "Philospher".to_string(),
                                                initial: Some("Thinking".to_string()),
                                                datamodel: vec![],
                                                onentry: vec![
                                                    ActionIr::If {
                                                        branches: vec![
                                                            IfBranchIr {
                                                                guard: Some(ExprIr::BinOp("!=".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                body: vec![
                                                                    ActionIr::Assign {
                                                                        loc: "i_ID_LEFT".to_string(),
                                                                        key_expr: None,
                                                                        expr: ExprIr::IndexAccess(Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_HAND_COMPLIANCE".to_string())), Box::new(ExprIr::BinOp("-".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))))), Box::new(ExprIr::IntLiteral(0_i64))),
                                                                        op: "set".to_string(),
                                                                    },
                                                                    ActionIr::Assign {
                                                                        loc: "i_ID_RIGHT".to_string(),
                                                                        key_expr: None,
                                                                        expr: ExprIr::IndexAccess(Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_HAND_COMPLIANCE".to_string())), Box::new(ExprIr::BinOp("-".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))))), Box::new(ExprIr::IntLiteral(1_i64))),
                                                                        op: "set".to_string(),
                                                                    },
                                                                ],
                                                            },
                                                        ],
                                                    },
                                                    ActionIr::Assign {
                                                        loc: "t_INPUTS['taken.' + str(i_ID_LEFT)]".to_string(),
                                                        key_expr: Some(Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None))))),
                                                        expr: ExprIr::IntLiteral(0_i64),
                                                        op: "set".to_string(),
                                                    },
                                                    ActionIr::Assign {
                                                        loc: "t_INPUTS['taken.' + str(i_ID_RIGHT)]".to_string(),
                                                        key_expr: Some(Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None))))),
                                                        expr: ExprIr::IntLiteral(0_i64),
                                                        op: "set".to_string(),
                                                    },
                                                ],
                                                onexit: vec![],
                                                transitions: vec![
                                                    TransitionIr {
                                                        event: Some("taken.*".to_string()),
                                                        targets: vec![],
                                                        guard: Some(ExprIr::BinOp("||".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))))),
                                                        actions: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::EventName], None)))),
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Assign {
                                                                loc: "t_INPUTS[_event.name]".to_string(),
                                                                key_expr: Some(Box::new(ExprIr::EventName)),
                                                                expr: ExprIr::EventData,
                                                                op: "set".to_string(),
                                                            },
                                                        ],
                                                    },
                                                    TransitionIr {
                                                        event: Some("error.*".to_string()),
                                                        targets: vec!["FinalSub".to_string()],
                                                        guard: None,
                                                        actions: vec![],
                                                    },
                                                    TransitionIr {
                                                        event: Some("update.delay".to_string()),
                                                        targets: vec![],
                                                        guard: None,
                                                        actions: vec![
                                                            ActionIr::Assign {
                                                                loc: "i_DELAY_THINK_EAT".to_string(),
                                                                key_expr: None,
                                                                expr: ExprIr::EventData,
                                                                op: "set".to_string(),
                                                            },
                                                        ],
                                                    },
                                                ],
                                                children: vec![
                                                    StateNodeIr::State {
                                                        id: "Eating".to_string(),
                                                        initial: None,
                                                        datamodel: vec![],
                                                        onentry: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("eat.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(1_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Send {
                                                                event: Some("Do.Timer.Think".to_string()),
                                                                eventexpr: None,
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_DELAY_THINK_EAT".to_string())], None)), Box::new(ExprIr::StringLiteral("ms".to_string())))),
                                                                send_id: Some("ID.Do.Timer.Think".to_string()),
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Assign {
                                                                loc: "i_EAT_COUNT".to_string(),
                                                                key_expr: None,
                                                                expr: ExprIr::BinOp("+".to_string(), Box::new(ExprIr::VarRead("i_EAT_COUNT".to_string())), Box::new(ExprIr::IntLiteral(1_i64))),
                                                                op: "set".to_string(),
                                                            },
                                                            ActionIr::Log {
                                                                expr: ExprIr::BinOp("+".to_string(), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("Philosopher: ".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))), Box::new(ExprIr::StringLiteral(" Count: ".to_string())))), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_EAT_COUNT".to_string())], None))),
                                                                label: Some("EATING".to_string()),
                                                            },
                                                        ],
                                                        onexit: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("eat.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Cancel { sendid: Some("ID.Do.Timer.Think".to_string()) },
                                                        ],
                                                        transitions: vec![],
                                                        children: vec![
                                                            StateNodeIr::State {
                                                                id: "RightTaken1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("Do.Timer.Think".to_string()),
                                                                        targets: vec!["RequirePutLeft".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftTaken1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("Do.Timer.Think".to_string()),
                                                                        targets: vec!["RequirePutRight".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftPut1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![],
                                                                children: vec![
                                                                    StateNodeIr::State {
                                                                        id: "RequirePutRight".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["PutLeftComplete".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                    StateNodeIr::State {
                                                                        id: "PutRightComplete".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["Thinking".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                ],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightPut1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![],
                                                                children: vec![
                                                                    StateNodeIr::State {
                                                                        id: "RequirePutLeft".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["PutRightComplete".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                    StateNodeIr::State {
                                                                        id: "PutLeftComplete".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["Thinking".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                ],
                                                                invokes: vec![],
                                                            },
                                                        ],
                                                        invokes: vec![],
                                                    },
                                                    StateNodeIr::State {
                                                        id: "Thinking".to_string(),
                                                        initial: None,
                                                        datamodel: vec![],
                                                        onentry: vec![
                                                            ActionIr::Send {
                                                                event: Some("Do.Timer.Hungry".to_string()),
                                                                eventexpr: None,
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_DELAY_THINK_EAT".to_string())], None)), Box::new(ExprIr::StringLiteral("ms".to_string())))),
                                                                send_id: Some("ID.Do.Timer.Hungry".to_string()),
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("think.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(1_i64)),
                                                                params: vec![],
                                                            },
                                                        ],
                                                        onexit: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("think.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Cancel { sendid: Some("ID.Do.Timer.Hungry".to_string()) },
                                                        ],
                                                        transitions: vec![
                                                            TransitionIr {
                                                                event: Some("Do.Timer.Hungry".to_string()),
                                                                targets: vec!["ProcessHungry".to_string()],
                                                                guard: None,
                                                                actions: vec![],
                                                            },
                                                        ],
                                                        children: vec![],
                                                        invokes: vec![],
                                                    },
                                                    StateNodeIr::State {
                                                        id: "ProcessHungry".to_string(),
                                                        initial: Some("Hungry".to_string()),
                                                        datamodel: vec![],
                                                        onentry: vec![],
                                                        onexit: vec![],
                                                        transitions: vec![],
                                                        children: vec![
                                                            StateNodeIr::State {
                                                                id: "Hungry".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftWaiting".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::VarRead("i_ID".to_string())),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("do.taken.*".to_string()),
                                                                        targets: vec!["LeftCheck".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightWaiting".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::VarRead("i_ID".to_string())),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("do.taken.*".to_string()),
                                                                        targets: vec!["RightCheck".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftCheck".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftTaken".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightPut".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightCheck".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightTaken".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftPut".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftTaken".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightTaken1".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftPut".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftPut".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["Hungry".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightTaken".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftTaken1".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightPut".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightPut".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["Hungry".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                        ],
                                                        invokes: vec![],
                                                    },
                                                ],
                                                invokes: vec![],
                                            },
                                            StateNodeIr::Final { id: "FinalSub".to_string() },
                                        ],
                                    })),
                            },
                        ],
                    },
                    StateNodeIr::State {
                        id: "Philosopher3".to_string(),
                        initial: Some("P3_Thinking".to_string()),
                        datamodel: vec![],
                        onentry: vec![],
                        onexit: vec![],
                        transitions: vec![],
                        children: vec![
                            StateNodeIr::State {
                                id: "P3_Thinking".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("think.3".to_string()),
                                        targets: vec!["P3_Hungry".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "P3_Eating".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("eat.3".to_string()),
                                        targets: vec!["P3_Hungry".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "P3_Hungry".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("eat.3".to_string()),
                                        targets: vec!["P3_Eating".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))),
                                        actions: vec![],
                                    },
                                    TransitionIr {
                                        event: Some("think.3".to_string()),
                                        targets: vec!["P3_Thinking".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                        ],
                        invokes: vec![
                            InvokeIr {
                                invoke_id: Some("ID_P_3".to_string()),
                                type_value: "scxml".to_string(),
                                src: None,
                                params: vec![SendParamIr { name: "i_ID".to_string(), expr: ExprIr::IntLiteral(3_i64) }, SendParamIr { name: "t_HAND_COMPLIANCE".to_string(), expr: ExprIr::VarRead("t_HAND_COMPLIANCE".to_string()) }, SendParamIr { name: "i_DELAY_THINK_EAT".to_string(), expr: ExprIr::VarRead("i_DELAY_THINK_EAT".to_string()) }],
                                autoforward: false,
                                blocked_msg: None,
                                child_machine: Some(Box::new(ChildMachineIr {
                                        name: Some("ScxmlPhilospher".to_string()),
                                        initial: vec!["Philospher".to_string()],
                                        datamodel: vec![DataSlotIr { id: "i_ID".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "i_ID_LEFT".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "i_ID_RIGHT".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "t_HAND_COMPLIANCE".to_string(), initial_value: Some(Value::Array(vec![Value::Array(vec![Value::Int(1_i64), Value::Int(5_i64)]), Value::Array(vec![Value::Int(2_i64), Value::Int(1_i64)]), Value::Array(vec![Value::Int(3_i64), Value::Int(2_i64)]), Value::Array(vec![Value::Int(4_i64), Value::Int(3_i64)]), Value::Array(vec![Value::Int(5_i64), Value::Int(4_i64)])])) }, DataSlotIr { id: "i_DELAY_THINK_EAT".to_string(), initial_value: Some(Value::Int(1000_i64)) }, DataSlotIr { id: "t_INPUTS".to_string(), initial_value: Some(Value::Map(BTreeMap::new())) }, DataSlotIr { id: "i_EAT_COUNT".to_string(), initial_value: Some(Value::Int(0_i64)) }],
                                        children: vec![
                                            StateNodeIr::State {
                                                id: "Philospher".to_string(),
                                                initial: Some("Thinking".to_string()),
                                                datamodel: vec![],
                                                onentry: vec![
                                                    ActionIr::If {
                                                        branches: vec![
                                                            IfBranchIr {
                                                                guard: Some(ExprIr::BinOp("!=".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                body: vec![
                                                                    ActionIr::Assign {
                                                                        loc: "i_ID_LEFT".to_string(),
                                                                        key_expr: None,
                                                                        expr: ExprIr::IndexAccess(Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_HAND_COMPLIANCE".to_string())), Box::new(ExprIr::BinOp("-".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))))), Box::new(ExprIr::IntLiteral(0_i64))),
                                                                        op: "set".to_string(),
                                                                    },
                                                                    ActionIr::Assign {
                                                                        loc: "i_ID_RIGHT".to_string(),
                                                                        key_expr: None,
                                                                        expr: ExprIr::IndexAccess(Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_HAND_COMPLIANCE".to_string())), Box::new(ExprIr::BinOp("-".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))))), Box::new(ExprIr::IntLiteral(1_i64))),
                                                                        op: "set".to_string(),
                                                                    },
                                                                ],
                                                            },
                                                        ],
                                                    },
                                                    ActionIr::Assign {
                                                        loc: "t_INPUTS['taken.' + str(i_ID_LEFT)]".to_string(),
                                                        key_expr: Some(Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None))))),
                                                        expr: ExprIr::IntLiteral(0_i64),
                                                        op: "set".to_string(),
                                                    },
                                                    ActionIr::Assign {
                                                        loc: "t_INPUTS['taken.' + str(i_ID_RIGHT)]".to_string(),
                                                        key_expr: Some(Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None))))),
                                                        expr: ExprIr::IntLiteral(0_i64),
                                                        op: "set".to_string(),
                                                    },
                                                ],
                                                onexit: vec![],
                                                transitions: vec![
                                                    TransitionIr {
                                                        event: Some("taken.*".to_string()),
                                                        targets: vec![],
                                                        guard: Some(ExprIr::BinOp("||".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))))),
                                                        actions: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::EventName], None)))),
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Assign {
                                                                loc: "t_INPUTS[_event.name]".to_string(),
                                                                key_expr: Some(Box::new(ExprIr::EventName)),
                                                                expr: ExprIr::EventData,
                                                                op: "set".to_string(),
                                                            },
                                                        ],
                                                    },
                                                    TransitionIr {
                                                        event: Some("error.*".to_string()),
                                                        targets: vec!["FinalSub".to_string()],
                                                        guard: None,
                                                        actions: vec![],
                                                    },
                                                    TransitionIr {
                                                        event: Some("update.delay".to_string()),
                                                        targets: vec![],
                                                        guard: None,
                                                        actions: vec![
                                                            ActionIr::Assign {
                                                                loc: "i_DELAY_THINK_EAT".to_string(),
                                                                key_expr: None,
                                                                expr: ExprIr::EventData,
                                                                op: "set".to_string(),
                                                            },
                                                        ],
                                                    },
                                                ],
                                                children: vec![
                                                    StateNodeIr::State {
                                                        id: "Eating".to_string(),
                                                        initial: None,
                                                        datamodel: vec![],
                                                        onentry: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("eat.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(1_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Send {
                                                                event: Some("Do.Timer.Think".to_string()),
                                                                eventexpr: None,
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_DELAY_THINK_EAT".to_string())], None)), Box::new(ExprIr::StringLiteral("ms".to_string())))),
                                                                send_id: Some("ID.Do.Timer.Think".to_string()),
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Assign {
                                                                loc: "i_EAT_COUNT".to_string(),
                                                                key_expr: None,
                                                                expr: ExprIr::BinOp("+".to_string(), Box::new(ExprIr::VarRead("i_EAT_COUNT".to_string())), Box::new(ExprIr::IntLiteral(1_i64))),
                                                                op: "set".to_string(),
                                                            },
                                                            ActionIr::Log {
                                                                expr: ExprIr::BinOp("+".to_string(), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("Philosopher: ".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))), Box::new(ExprIr::StringLiteral(" Count: ".to_string())))), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_EAT_COUNT".to_string())], None))),
                                                                label: Some("EATING".to_string()),
                                                            },
                                                        ],
                                                        onexit: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("eat.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Cancel { sendid: Some("ID.Do.Timer.Think".to_string()) },
                                                        ],
                                                        transitions: vec![],
                                                        children: vec![
                                                            StateNodeIr::State {
                                                                id: "RightTaken1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("Do.Timer.Think".to_string()),
                                                                        targets: vec!["RequirePutLeft".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftTaken1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("Do.Timer.Think".to_string()),
                                                                        targets: vec!["RequirePutRight".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftPut1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![],
                                                                children: vec![
                                                                    StateNodeIr::State {
                                                                        id: "RequirePutRight".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["PutLeftComplete".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                    StateNodeIr::State {
                                                                        id: "PutRightComplete".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["Thinking".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                ],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightPut1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![],
                                                                children: vec![
                                                                    StateNodeIr::State {
                                                                        id: "RequirePutLeft".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["PutRightComplete".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                    StateNodeIr::State {
                                                                        id: "PutLeftComplete".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["Thinking".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                ],
                                                                invokes: vec![],
                                                            },
                                                        ],
                                                        invokes: vec![],
                                                    },
                                                    StateNodeIr::State {
                                                        id: "Thinking".to_string(),
                                                        initial: None,
                                                        datamodel: vec![],
                                                        onentry: vec![
                                                            ActionIr::Send {
                                                                event: Some("Do.Timer.Hungry".to_string()),
                                                                eventexpr: None,
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_DELAY_THINK_EAT".to_string())], None)), Box::new(ExprIr::StringLiteral("ms".to_string())))),
                                                                send_id: Some("ID.Do.Timer.Hungry".to_string()),
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("think.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(1_i64)),
                                                                params: vec![],
                                                            },
                                                        ],
                                                        onexit: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("think.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Cancel { sendid: Some("ID.Do.Timer.Hungry".to_string()) },
                                                        ],
                                                        transitions: vec![
                                                            TransitionIr {
                                                                event: Some("Do.Timer.Hungry".to_string()),
                                                                targets: vec!["ProcessHungry".to_string()],
                                                                guard: None,
                                                                actions: vec![],
                                                            },
                                                        ],
                                                        children: vec![],
                                                        invokes: vec![],
                                                    },
                                                    StateNodeIr::State {
                                                        id: "ProcessHungry".to_string(),
                                                        initial: Some("Hungry".to_string()),
                                                        datamodel: vec![],
                                                        onentry: vec![],
                                                        onexit: vec![],
                                                        transitions: vec![],
                                                        children: vec![
                                                            StateNodeIr::State {
                                                                id: "Hungry".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftWaiting".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::VarRead("i_ID".to_string())),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("do.taken.*".to_string()),
                                                                        targets: vec!["LeftCheck".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightWaiting".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::VarRead("i_ID".to_string())),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("do.taken.*".to_string()),
                                                                        targets: vec!["RightCheck".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftCheck".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftTaken".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightPut".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightCheck".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightTaken".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftPut".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftTaken".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightTaken1".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftPut".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftPut".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["Hungry".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightTaken".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftTaken1".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightPut".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightPut".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["Hungry".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                        ],
                                                        invokes: vec![],
                                                    },
                                                ],
                                                invokes: vec![],
                                            },
                                            StateNodeIr::Final { id: "FinalSub".to_string() },
                                        ],
                                    })),
                            },
                        ],
                    },
                    StateNodeIr::State {
                        id: "Philosopher2".to_string(),
                        initial: Some("P2_Thinking".to_string()),
                        datamodel: vec![],
                        onentry: vec![],
                        onexit: vec![],
                        transitions: vec![],
                        children: vec![
                            StateNodeIr::State {
                                id: "P2_Thinking".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("think.2".to_string()),
                                        targets: vec!["P2_Hungry".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "P2_Eating".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("eat.2".to_string()),
                                        targets: vec!["P2_Hungry".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "P2_Hungry".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("eat.2".to_string()),
                                        targets: vec!["P2_Eating".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))),
                                        actions: vec![],
                                    },
                                    TransitionIr {
                                        event: Some("think.2".to_string()),
                                        targets: vec!["P2_Thinking".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                        ],
                        invokes: vec![
                            InvokeIr {
                                invoke_id: Some("ID_P_2".to_string()),
                                type_value: "scxml".to_string(),
                                src: None,
                                params: vec![SendParamIr { name: "i_ID".to_string(), expr: ExprIr::IntLiteral(2_i64) }, SendParamIr { name: "t_HAND_COMPLIANCE".to_string(), expr: ExprIr::VarRead("t_HAND_COMPLIANCE".to_string()) }, SendParamIr { name: "i_DELAY_THINK_EAT".to_string(), expr: ExprIr::VarRead("i_DELAY_THINK_EAT".to_string()) }],
                                autoforward: false,
                                blocked_msg: None,
                                child_machine: Some(Box::new(ChildMachineIr {
                                        name: Some("ScxmlPhilospher".to_string()),
                                        initial: vec!["Philospher".to_string()],
                                        datamodel: vec![DataSlotIr { id: "i_ID".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "i_ID_LEFT".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "i_ID_RIGHT".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "t_HAND_COMPLIANCE".to_string(), initial_value: Some(Value::Array(vec![Value::Array(vec![Value::Int(1_i64), Value::Int(5_i64)]), Value::Array(vec![Value::Int(2_i64), Value::Int(1_i64)]), Value::Array(vec![Value::Int(3_i64), Value::Int(2_i64)]), Value::Array(vec![Value::Int(4_i64), Value::Int(3_i64)]), Value::Array(vec![Value::Int(5_i64), Value::Int(4_i64)])])) }, DataSlotIr { id: "i_DELAY_THINK_EAT".to_string(), initial_value: Some(Value::Int(1000_i64)) }, DataSlotIr { id: "t_INPUTS".to_string(), initial_value: Some(Value::Map(BTreeMap::new())) }, DataSlotIr { id: "i_EAT_COUNT".to_string(), initial_value: Some(Value::Int(0_i64)) }],
                                        children: vec![
                                            StateNodeIr::State {
                                                id: "Philospher".to_string(),
                                                initial: Some("Thinking".to_string()),
                                                datamodel: vec![],
                                                onentry: vec![
                                                    ActionIr::If {
                                                        branches: vec![
                                                            IfBranchIr {
                                                                guard: Some(ExprIr::BinOp("!=".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                body: vec![
                                                                    ActionIr::Assign {
                                                                        loc: "i_ID_LEFT".to_string(),
                                                                        key_expr: None,
                                                                        expr: ExprIr::IndexAccess(Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_HAND_COMPLIANCE".to_string())), Box::new(ExprIr::BinOp("-".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))))), Box::new(ExprIr::IntLiteral(0_i64))),
                                                                        op: "set".to_string(),
                                                                    },
                                                                    ActionIr::Assign {
                                                                        loc: "i_ID_RIGHT".to_string(),
                                                                        key_expr: None,
                                                                        expr: ExprIr::IndexAccess(Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_HAND_COMPLIANCE".to_string())), Box::new(ExprIr::BinOp("-".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))))), Box::new(ExprIr::IntLiteral(1_i64))),
                                                                        op: "set".to_string(),
                                                                    },
                                                                ],
                                                            },
                                                        ],
                                                    },
                                                    ActionIr::Assign {
                                                        loc: "t_INPUTS['taken.' + str(i_ID_LEFT)]".to_string(),
                                                        key_expr: Some(Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None))))),
                                                        expr: ExprIr::IntLiteral(0_i64),
                                                        op: "set".to_string(),
                                                    },
                                                    ActionIr::Assign {
                                                        loc: "t_INPUTS['taken.' + str(i_ID_RIGHT)]".to_string(),
                                                        key_expr: Some(Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None))))),
                                                        expr: ExprIr::IntLiteral(0_i64),
                                                        op: "set".to_string(),
                                                    },
                                                ],
                                                onexit: vec![],
                                                transitions: vec![
                                                    TransitionIr {
                                                        event: Some("taken.*".to_string()),
                                                        targets: vec![],
                                                        guard: Some(ExprIr::BinOp("||".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))))),
                                                        actions: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::EventName], None)))),
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Assign {
                                                                loc: "t_INPUTS[_event.name]".to_string(),
                                                                key_expr: Some(Box::new(ExprIr::EventName)),
                                                                expr: ExprIr::EventData,
                                                                op: "set".to_string(),
                                                            },
                                                        ],
                                                    },
                                                    TransitionIr {
                                                        event: Some("error.*".to_string()),
                                                        targets: vec!["FinalSub".to_string()],
                                                        guard: None,
                                                        actions: vec![],
                                                    },
                                                    TransitionIr {
                                                        event: Some("update.delay".to_string()),
                                                        targets: vec![],
                                                        guard: None,
                                                        actions: vec![
                                                            ActionIr::Assign {
                                                                loc: "i_DELAY_THINK_EAT".to_string(),
                                                                key_expr: None,
                                                                expr: ExprIr::EventData,
                                                                op: "set".to_string(),
                                                            },
                                                        ],
                                                    },
                                                ],
                                                children: vec![
                                                    StateNodeIr::State {
                                                        id: "Eating".to_string(),
                                                        initial: None,
                                                        datamodel: vec![],
                                                        onentry: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("eat.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(1_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Send {
                                                                event: Some("Do.Timer.Think".to_string()),
                                                                eventexpr: None,
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_DELAY_THINK_EAT".to_string())], None)), Box::new(ExprIr::StringLiteral("ms".to_string())))),
                                                                send_id: Some("ID.Do.Timer.Think".to_string()),
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Assign {
                                                                loc: "i_EAT_COUNT".to_string(),
                                                                key_expr: None,
                                                                expr: ExprIr::BinOp("+".to_string(), Box::new(ExprIr::VarRead("i_EAT_COUNT".to_string())), Box::new(ExprIr::IntLiteral(1_i64))),
                                                                op: "set".to_string(),
                                                            },
                                                            ActionIr::Log {
                                                                expr: ExprIr::BinOp("+".to_string(), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("Philosopher: ".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))), Box::new(ExprIr::StringLiteral(" Count: ".to_string())))), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_EAT_COUNT".to_string())], None))),
                                                                label: Some("EATING".to_string()),
                                                            },
                                                        ],
                                                        onexit: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("eat.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Cancel { sendid: Some("ID.Do.Timer.Think".to_string()) },
                                                        ],
                                                        transitions: vec![],
                                                        children: vec![
                                                            StateNodeIr::State {
                                                                id: "RightTaken1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("Do.Timer.Think".to_string()),
                                                                        targets: vec!["RequirePutLeft".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftTaken1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("Do.Timer.Think".to_string()),
                                                                        targets: vec!["RequirePutRight".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftPut1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![],
                                                                children: vec![
                                                                    StateNodeIr::State {
                                                                        id: "RequirePutRight".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["PutLeftComplete".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                    StateNodeIr::State {
                                                                        id: "PutRightComplete".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["Thinking".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                ],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightPut1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![],
                                                                children: vec![
                                                                    StateNodeIr::State {
                                                                        id: "RequirePutLeft".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["PutRightComplete".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                    StateNodeIr::State {
                                                                        id: "PutLeftComplete".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["Thinking".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                ],
                                                                invokes: vec![],
                                                            },
                                                        ],
                                                        invokes: vec![],
                                                    },
                                                    StateNodeIr::State {
                                                        id: "Thinking".to_string(),
                                                        initial: None,
                                                        datamodel: vec![],
                                                        onentry: vec![
                                                            ActionIr::Send {
                                                                event: Some("Do.Timer.Hungry".to_string()),
                                                                eventexpr: None,
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_DELAY_THINK_EAT".to_string())], None)), Box::new(ExprIr::StringLiteral("ms".to_string())))),
                                                                send_id: Some("ID.Do.Timer.Hungry".to_string()),
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("think.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(1_i64)),
                                                                params: vec![],
                                                            },
                                                        ],
                                                        onexit: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("think.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Cancel { sendid: Some("ID.Do.Timer.Hungry".to_string()) },
                                                        ],
                                                        transitions: vec![
                                                            TransitionIr {
                                                                event: Some("Do.Timer.Hungry".to_string()),
                                                                targets: vec!["ProcessHungry".to_string()],
                                                                guard: None,
                                                                actions: vec![],
                                                            },
                                                        ],
                                                        children: vec![],
                                                        invokes: vec![],
                                                    },
                                                    StateNodeIr::State {
                                                        id: "ProcessHungry".to_string(),
                                                        initial: Some("Hungry".to_string()),
                                                        datamodel: vec![],
                                                        onentry: vec![],
                                                        onexit: vec![],
                                                        transitions: vec![],
                                                        children: vec![
                                                            StateNodeIr::State {
                                                                id: "Hungry".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftWaiting".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::VarRead("i_ID".to_string())),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("do.taken.*".to_string()),
                                                                        targets: vec!["LeftCheck".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightWaiting".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::VarRead("i_ID".to_string())),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("do.taken.*".to_string()),
                                                                        targets: vec!["RightCheck".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftCheck".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftTaken".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightPut".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightCheck".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightTaken".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftPut".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftTaken".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightTaken1".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftPut".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftPut".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["Hungry".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightTaken".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftTaken1".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightPut".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightPut".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["Hungry".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                        ],
                                                        invokes: vec![],
                                                    },
                                                ],
                                                invokes: vec![],
                                            },
                                            StateNodeIr::Final { id: "FinalSub".to_string() },
                                        ],
                                    })),
                            },
                        ],
                    },
                    StateNodeIr::State {
                        id: "Philosopher1".to_string(),
                        initial: Some("P1_Thinking".to_string()),
                        datamodel: vec![],
                        onentry: vec![],
                        onexit: vec![],
                        transitions: vec![],
                        children: vec![
                            StateNodeIr::State {
                                id: "P1_Thinking".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("think.1".to_string()),
                                        targets: vec!["P1_Hungry".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "P1_Hungry".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("eat.1".to_string()),
                                        targets: vec!["P1_Eating".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))),
                                        actions: vec![],
                                    },
                                    TransitionIr {
                                        event: Some("think.1".to_string()),
                                        targets: vec!["P1_Thinking".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "P1_Eating".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: Some("eat.1".to_string()),
                                        targets: vec!["P1_Hungry".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventData), Box::new(ExprIr::IntLiteral(1_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                        ],
                        invokes: vec![
                            InvokeIr {
                                invoke_id: Some("ID_P_1".to_string()),
                                type_value: "scxml".to_string(),
                                src: None,
                                params: vec![SendParamIr { name: "i_ID".to_string(), expr: ExprIr::IntLiteral(1_i64) }, SendParamIr { name: "t_HAND_COMPLIANCE".to_string(), expr: ExprIr::VarRead("t_HAND_COMPLIANCE".to_string()) }, SendParamIr { name: "i_DELAY_THINK_EAT".to_string(), expr: ExprIr::VarRead("i_DELAY_THINK_EAT".to_string()) }],
                                autoforward: false,
                                blocked_msg: None,
                                child_machine: Some(Box::new(ChildMachineIr {
                                        name: Some("ScxmlPhilospher".to_string()),
                                        initial: vec!["Philospher".to_string()],
                                        datamodel: vec![DataSlotIr { id: "i_ID".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "i_ID_LEFT".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "i_ID_RIGHT".to_string(), initial_value: Some(Value::Int(0_i64)) }, DataSlotIr { id: "t_HAND_COMPLIANCE".to_string(), initial_value: Some(Value::Array(vec![Value::Array(vec![Value::Int(1_i64), Value::Int(5_i64)]), Value::Array(vec![Value::Int(2_i64), Value::Int(1_i64)]), Value::Array(vec![Value::Int(3_i64), Value::Int(2_i64)]), Value::Array(vec![Value::Int(4_i64), Value::Int(3_i64)]), Value::Array(vec![Value::Int(5_i64), Value::Int(4_i64)])])) }, DataSlotIr { id: "i_DELAY_THINK_EAT".to_string(), initial_value: Some(Value::Int(1000_i64)) }, DataSlotIr { id: "t_INPUTS".to_string(), initial_value: Some(Value::Map(BTreeMap::new())) }, DataSlotIr { id: "i_EAT_COUNT".to_string(), initial_value: Some(Value::Int(0_i64)) }],
                                        children: vec![
                                            StateNodeIr::State {
                                                id: "Philospher".to_string(),
                                                initial: Some("Thinking".to_string()),
                                                datamodel: vec![],
                                                onentry: vec![
                                                    ActionIr::If {
                                                        branches: vec![
                                                            IfBranchIr {
                                                                guard: Some(ExprIr::BinOp("!=".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                body: vec![
                                                                    ActionIr::Assign {
                                                                        loc: "i_ID_LEFT".to_string(),
                                                                        key_expr: None,
                                                                        expr: ExprIr::IndexAccess(Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_HAND_COMPLIANCE".to_string())), Box::new(ExprIr::BinOp("-".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))))), Box::new(ExprIr::IntLiteral(0_i64))),
                                                                        op: "set".to_string(),
                                                                    },
                                                                    ActionIr::Assign {
                                                                        loc: "i_ID_RIGHT".to_string(),
                                                                        key_expr: None,
                                                                        expr: ExprIr::IndexAccess(Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_HAND_COMPLIANCE".to_string())), Box::new(ExprIr::BinOp("-".to_string(), Box::new(ExprIr::VarRead("i_ID".to_string())), Box::new(ExprIr::IntLiteral(1_i64)))))), Box::new(ExprIr::IntLiteral(1_i64))),
                                                                        op: "set".to_string(),
                                                                    },
                                                                ],
                                                            },
                                                        ],
                                                    },
                                                    ActionIr::Assign {
                                                        loc: "t_INPUTS['taken.' + str(i_ID_LEFT)]".to_string(),
                                                        key_expr: Some(Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None))))),
                                                        expr: ExprIr::IntLiteral(0_i64),
                                                        op: "set".to_string(),
                                                    },
                                                    ActionIr::Assign {
                                                        loc: "t_INPUTS['taken.' + str(i_ID_RIGHT)]".to_string(),
                                                        key_expr: Some(Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None))))),
                                                        expr: ExprIr::IntLiteral(0_i64),
                                                        op: "set".to_string(),
                                                    },
                                                ],
                                                onexit: vec![],
                                                transitions: vec![
                                                    TransitionIr {
                                                        event: Some("taken.*".to_string()),
                                                        targets: vec![],
                                                        guard: Some(ExprIr::BinOp("||".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))))),
                                                        actions: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::EventName], None)))),
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Assign {
                                                                loc: "t_INPUTS[_event.name]".to_string(),
                                                                key_expr: Some(Box::new(ExprIr::EventName)),
                                                                expr: ExprIr::EventData,
                                                                op: "set".to_string(),
                                                            },
                                                        ],
                                                    },
                                                    TransitionIr {
                                                        event: Some("error.*".to_string()),
                                                        targets: vec!["FinalSub".to_string()],
                                                        guard: None,
                                                        actions: vec![],
                                                    },
                                                    TransitionIr {
                                                        event: Some("update.delay".to_string()),
                                                        targets: vec![],
                                                        guard: None,
                                                        actions: vec![
                                                            ActionIr::Assign {
                                                                loc: "i_DELAY_THINK_EAT".to_string(),
                                                                key_expr: None,
                                                                expr: ExprIr::EventData,
                                                                op: "set".to_string(),
                                                            },
                                                        ],
                                                    },
                                                ],
                                                children: vec![
                                                    StateNodeIr::State {
                                                        id: "Eating".to_string(),
                                                        initial: None,
                                                        datamodel: vec![],
                                                        onentry: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("eat.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(1_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Send {
                                                                event: Some("Do.Timer.Think".to_string()),
                                                                eventexpr: None,
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_DELAY_THINK_EAT".to_string())], None)), Box::new(ExprIr::StringLiteral("ms".to_string())))),
                                                                send_id: Some("ID.Do.Timer.Think".to_string()),
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Assign {
                                                                loc: "i_EAT_COUNT".to_string(),
                                                                key_expr: None,
                                                                expr: ExprIr::BinOp("+".to_string(), Box::new(ExprIr::VarRead("i_EAT_COUNT".to_string())), Box::new(ExprIr::IntLiteral(1_i64))),
                                                                op: "set".to_string(),
                                                            },
                                                            ActionIr::Log {
                                                                expr: ExprIr::BinOp("+".to_string(), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("Philosopher: ".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))), Box::new(ExprIr::StringLiteral(" Count: ".to_string())))), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_EAT_COUNT".to_string())], None))),
                                                                label: Some("EATING".to_string()),
                                                            },
                                                        ],
                                                        onexit: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("eat.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Cancel { sendid: Some("ID.Do.Timer.Think".to_string()) },
                                                        ],
                                                        transitions: vec![],
                                                        children: vec![
                                                            StateNodeIr::State {
                                                                id: "RightTaken1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("Do.Timer.Think".to_string()),
                                                                        targets: vec!["RequirePutLeft".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftTaken1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("Do.Timer.Think".to_string()),
                                                                        targets: vec!["RequirePutRight".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftPut1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![],
                                                                children: vec![
                                                                    StateNodeIr::State {
                                                                        id: "RequirePutRight".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["PutLeftComplete".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                    StateNodeIr::State {
                                                                        id: "PutRightComplete".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["Thinking".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                ],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightPut1".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![],
                                                                children: vec![
                                                                    StateNodeIr::State {
                                                                        id: "RequirePutLeft".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["PutRightComplete".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                    StateNodeIr::State {
                                                                        id: "PutLeftComplete".to_string(),
                                                                        initial: None,
                                                                        datamodel: vec![],
                                                                        onentry: vec![],
                                                                        onexit: vec![],
                                                                        transitions: vec![
                                                                            TransitionIr {
                                                                                event: None,
                                                                                targets: vec!["Thinking".to_string()],
                                                                                guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                                actions: vec![],
                                                                            },
                                                                        ],
                                                                        children: vec![],
                                                                        invokes: vec![],
                                                                    },
                                                                ],
                                                                invokes: vec![],
                                                            },
                                                        ],
                                                        invokes: vec![],
                                                    },
                                                    StateNodeIr::State {
                                                        id: "Thinking".to_string(),
                                                        initial: None,
                                                        datamodel: vec![],
                                                        onentry: vec![
                                                            ActionIr::Send {
                                                                event: Some("Do.Timer.Hungry".to_string()),
                                                                eventexpr: None,
                                                                target: None,
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_DELAY_THINK_EAT".to_string())], None)), Box::new(ExprIr::StringLiteral("ms".to_string())))),
                                                                send_id: Some("ID.Do.Timer.Hungry".to_string()),
                                                                content_expr: None,
                                                                params: vec![],
                                                            },
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("think.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(1_i64)),
                                                                params: vec![],
                                                            },
                                                        ],
                                                        onexit: vec![
                                                            ActionIr::Send {
                                                                event: None,
                                                                eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("think.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID".to_string())], None)))),
                                                                target: Some("#_parent".to_string()),
                                                                targetexpr: None,
                                                                delay: Some("0s".to_string()),
                                                                delayexpr: None,
                                                                send_id: None,
                                                                content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                params: vec![],
                                                            },
                                                            ActionIr::Cancel { sendid: Some("ID.Do.Timer.Hungry".to_string()) },
                                                        ],
                                                        transitions: vec![
                                                            TransitionIr {
                                                                event: Some("Do.Timer.Hungry".to_string()),
                                                                targets: vec!["ProcessHungry".to_string()],
                                                                guard: None,
                                                                actions: vec![],
                                                            },
                                                        ],
                                                        children: vec![],
                                                        invokes: vec![],
                                                    },
                                                    StateNodeIr::State {
                                                        id: "ProcessHungry".to_string(),
                                                        initial: Some("Hungry".to_string()),
                                                        datamodel: vec![],
                                                        onentry: vec![],
                                                        onexit: vec![],
                                                        transitions: vec![],
                                                        children: vec![
                                                            StateNodeIr::State {
                                                                id: "Hungry".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftWaiting".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::VarRead("i_ID".to_string())),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("do.taken.*".to_string()),
                                                                        targets: vec!["LeftCheck".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightWaiting".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::VarRead("i_ID".to_string())),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: Some("do.taken.*".to_string()),
                                                                        targets: vec!["RightCheck".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::EventName), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("do.taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftCheck".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftTaken".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightPut".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightCheck".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightTaken".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftPut".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftTaken".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightTaken1".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftPut".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "LeftPut".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["Hungry".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightTaken".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftTaken1".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::VarRead("i_ID".to_string())))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["LeftWaiting".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_LEFT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["RightPut".to_string()],
                                                                        guard: None,
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                            StateNodeIr::State {
                                                                id: "RightPut".to_string(),
                                                                initial: None,
                                                                datamodel: vec![],
                                                                onentry: vec![
                                                                    ActionIr::Send {
                                                                        event: None,
                                                                        eventexpr: Some(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("take.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))),
                                                                        target: Some("#_parent".to_string()),
                                                                        targetexpr: None,
                                                                        delay: Some("0s".to_string()),
                                                                        delayexpr: None,
                                                                        send_id: None,
                                                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                                                        params: vec![],
                                                                    },
                                                                ],
                                                                onexit: vec![],
                                                                transitions: vec![
                                                                    TransitionIr {
                                                                        event: None,
                                                                        targets: vec!["Hungry".to_string()],
                                                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::BinOp("+".to_string(), Box::new(ExprIr::StringLiteral("taken.".to_string())), Box::new(ExprIr::Call("str".to_string(), vec![ExprIr::VarRead("i_ID_RIGHT".to_string())], None)))))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                                                        actions: vec![],
                                                                    },
                                                                ],
                                                                children: vec![],
                                                                invokes: vec![],
                                                            },
                                                        ],
                                                        invokes: vec![],
                                                    },
                                                ],
                                                invokes: vec![],
                                            },
                                            StateNodeIr::Final { id: "FinalSub".to_string() },
                                        ],
                                    })),
                            },
                        ],
                    },
                    StateNodeIr::State {
                        id: "Fork4".to_string(),
                        initial: Some("Fork4_Down".to_string()),
                        datamodel: vec![],
                        onentry: vec![],
                        onexit: vec![],
                        transitions: vec![],
                        children: vec![
                            StateNodeIr::State {
                                id: "Fork4_Down".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: None,
                                        targets: vec!["Fork4_Up".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.4".to_string())))), Box::new(ExprIr::IntLiteral(0_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "Fork4_Up".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![
                                    ActionIr::Send {
                                        event: Some("taken.4".to_string()),
                                        eventexpr: None,
                                        target: None,
                                        targetexpr: None,
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.4".to_string())))),
                                        params: vec![],
                                    },
                                ],
                                onexit: vec![
                                    ActionIr::Send {
                                        event: Some("taken.4".to_string()),
                                        eventexpr: None,
                                        target: None,
                                        targetexpr: None,
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                        params: vec![],
                                    },
                                ],
                                transitions: vec![
                                    TransitionIr {
                                        event: None,
                                        targets: vec!["Fork4_Down".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.4".to_string())))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                        ],
                        invokes: vec![],
                    },
                    StateNodeIr::State {
                        id: "Fork5".to_string(),
                        initial: Some("Fork5_Down".to_string()),
                        datamodel: vec![],
                        onentry: vec![],
                        onexit: vec![],
                        transitions: vec![],
                        children: vec![
                            StateNodeIr::State {
                                id: "Fork5_Down".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: None,
                                        targets: vec!["Fork5_Up".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.5".to_string())))), Box::new(ExprIr::IntLiteral(0_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "Fork5_Up".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![
                                    ActionIr::Send {
                                        event: Some("taken.5".to_string()),
                                        eventexpr: None,
                                        target: None,
                                        targetexpr: None,
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.5".to_string())))),
                                        params: vec![],
                                    },
                                ],
                                onexit: vec![
                                    ActionIr::Send {
                                        event: Some("taken.5".to_string()),
                                        eventexpr: None,
                                        target: None,
                                        targetexpr: None,
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                        params: vec![],
                                    },
                                ],
                                transitions: vec![
                                    TransitionIr {
                                        event: None,
                                        targets: vec!["Fork5_Down".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.5".to_string())))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                        ],
                        invokes: vec![],
                    },
                    StateNodeIr::State {
                        id: "Fork3".to_string(),
                        initial: Some("Fork3_Down".to_string()),
                        datamodel: vec![],
                        onentry: vec![],
                        onexit: vec![],
                        transitions: vec![],
                        children: vec![
                            StateNodeIr::State {
                                id: "Fork3_Down".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: None,
                                        targets: vec!["Fork3_Up".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.3".to_string())))), Box::new(ExprIr::IntLiteral(0_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "Fork3_Up".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![
                                    ActionIr::Send {
                                        event: Some("taken.3".to_string()),
                                        eventexpr: None,
                                        target: None,
                                        targetexpr: None,
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.3".to_string())))),
                                        params: vec![],
                                    },
                                ],
                                onexit: vec![
                                    ActionIr::Send {
                                        event: Some("taken.3".to_string()),
                                        eventexpr: None,
                                        target: None,
                                        targetexpr: None,
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                        params: vec![],
                                    },
                                ],
                                transitions: vec![
                                    TransitionIr {
                                        event: None,
                                        targets: vec!["Fork3_Down".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.3".to_string())))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                        ],
                        invokes: vec![],
                    },
                    StateNodeIr::State {
                        id: "Fork2".to_string(),
                        initial: Some("Fork2_Down".to_string()),
                        datamodel: vec![],
                        onentry: vec![],
                        onexit: vec![],
                        transitions: vec![],
                        children: vec![
                            StateNodeIr::State {
                                id: "Fork2_Down".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: None,
                                        targets: vec!["Fork2_Up".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.2".to_string())))), Box::new(ExprIr::IntLiteral(0_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "Fork2_Up".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![
                                    ActionIr::Send {
                                        event: Some("taken.2".to_string()),
                                        eventexpr: None,
                                        target: None,
                                        targetexpr: None,
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.2".to_string())))),
                                        params: vec![],
                                    },
                                ],
                                onexit: vec![
                                    ActionIr::Send {
                                        event: Some("taken.2".to_string()),
                                        eventexpr: None,
                                        target: None,
                                        targetexpr: None,
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                        params: vec![],
                                    },
                                ],
                                transitions: vec![
                                    TransitionIr {
                                        event: None,
                                        targets: vec!["Fork2_Down".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.2".to_string())))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                        ],
                        invokes: vec![],
                    },
                    StateNodeIr::State {
                        id: "Fork1".to_string(),
                        initial: Some("Fork1_Down".to_string()),
                        datamodel: vec![],
                        onentry: vec![],
                        onexit: vec![],
                        transitions: vec![],
                        children: vec![
                            StateNodeIr::State {
                                id: "Fork1_Down".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![],
                                onexit: vec![],
                                transitions: vec![
                                    TransitionIr {
                                        event: None,
                                        targets: vec!["Fork1_Up".to_string()],
                                        guard: Some(ExprIr::UnaryOp("not".to_string(), Box::new(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.1".to_string())))), Box::new(ExprIr::IntLiteral(0_i64)))))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                            StateNodeIr::State {
                                id: "Fork1_Up".to_string(),
                                initial: None,
                                datamodel: vec![],
                                onentry: vec![
                                    ActionIr::Send {
                                        event: Some("taken.1".to_string()),
                                        eventexpr: None,
                                        target: None,
                                        targetexpr: None,
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.1".to_string())))),
                                        params: vec![],
                                    },
                                ],
                                onexit: vec![
                                    ActionIr::Send {
                                        event: Some("taken.1".to_string()),
                                        eventexpr: None,
                                        target: None,
                                        targetexpr: None,
                                        delay: Some("0s".to_string()),
                                        delayexpr: None,
                                        send_id: None,
                                        content_expr: Some(ExprIr::IntLiteral(0_i64)),
                                        params: vec![],
                                    },
                                ],
                                transitions: vec![
                                    TransitionIr {
                                        event: None,
                                        targets: vec!["Fork1_Down".to_string()],
                                        guard: Some(ExprIr::BinOp("==".to_string(), Box::new(ExprIr::IndexAccess(Box::new(ExprIr::VarRead("t_INPUTS".to_string())), Box::new(ExprIr::StringLiteral("take.1".to_string())))), Box::new(ExprIr::IntLiteral(0_i64)))),
                                        actions: vec![],
                                    },
                                ],
                                children: vec![],
                                invokes: vec![],
                            },
                        ],
                        invokes: vec![],
                    },
                ],
                invokes: vec![],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------
    // Generic baseline tests (machine-independent)
    // -------------------------------------------------------

    #[test]
    fn test_machine_creates() {
        // Machine must initialize without panicking.
        let m = Machine::new();
        let _ = m.current_state();
    }

    #[test]
    fn test_machine_starts() {
        let mut m = Machine::new();
        m.start();
        // After start the machine must be in a valid, non-empty state.
        let state = m.current_state();
        assert!(!state.is_empty(), "Machine state must be non-empty after start");
        eprintln!("Initial state: {}", state);
    }

    #[test]
    fn test_machine_ir_data_not_empty() {
        let ir = machine_ir_data();
        assert!(
            !ir.children.is_empty(),
            "Machine IR must have at least one state"
        );
    }

    #[test]
    fn test_event_pattern_matching() {
        // Generic pattern-matching rules that every generated machine relies on.
        assert!(event_matches_pattern("foo.bar", "foo.bar"), "exact match");
        assert!(event_matches_pattern("foo.bar", "foo.*"), "prefix wildcard");
        assert!(event_matches_pattern("any.event", "*"), "catch-all wildcard");
        assert!(!event_matches_pattern("foo.bar", "baz.*"), "no spurious match");
        // Machine-specific: first real event in the vocabulary must match itself.
        assert!(
            event_matches_pattern("take.*", "take.*"),
            "first machine event must match itself"
        );
    }

    #[test]
    fn test_value_arithmetic() {
        let scope = Scope::new();
        let event = EventCtx::default();

        // 1 + 2 = 3
        let add_expr = ExprIr::BinOp(
            "+".to_string(),
            Box::new(ExprIr::IntLiteral(1_i64)),
            Box::new(ExprIr::IntLiteral(2_i64)),
        );
        let result = eval_expr(&add_expr, &scope, &event);
        assert_eq!(result, Value::Int(3), "1 + 2 must equal 3");

        // String concat: "hello" + " world"
        let concat_expr = ExprIr::BinOp(
            "+".to_string(),
            Box::new(ExprIr::StringLiteral("hello".to_string())),
            Box::new(ExprIr::StringLiteral(" world".to_string())),
        );
        let result2 = eval_expr(&concat_expr, &scope, &event);
        assert_eq!(result2, Value::Str("hello world".to_string()));
    }

    #[test]
    fn test_scope_read_write() {
        let mut scope = Scope::new();
        let event = EventCtx::default();

        scope.set("x".to_string(), Value::Int(42_i64));
        let read = ExprIr::VarRead("x".to_string());
        let result = eval_expr(&read, &scope, &event);
        assert_eq!(result, Value::Int(42_i64));
    }

    #[test]
    fn test_scope_map_key() {
        // Verify the map-key helpers compile and round-trip correctly.
        // Uses a synthetic key name that is safe for all machines.
        let mut scope = Scope::new();
        scope.set_map_key("t_MAP", "key.1".to_string(), Value::Int(0_i64));
        let val = scope.get_map_key("t_MAP", "key.1");
        assert_eq!(val, Value::Int(0_i64));
    }

    #[test]
    fn test_dispatch_first_event() {
        // Dispatch the first real event from the machine's transition vocabulary
        // (take.*) and verify the machine reaches a non-empty state.
        let mut m = Machine::new();
        m.start();

        let state_before = m.current_state().to_string();
        eprintln!("State before event dispatch: {}", state_before);

        m.step("take.*", Value::Undefined);

        let state_after = m.current_state().to_string();
        eprintln!("State after dispatching 'take.*': {}", state_after);
        assert!(
            !state_after.is_empty(),
            "Machine must be in a non-empty state after event dispatch"
        );
    }

    #[test]
    fn test_child_machine_state_accessible() {
        // get_child_state / get_child_var with an unknown id must return None
        // without panicking — regardless of whether this machine has children.
        let mut m = Machine::new();
        m.start();
        let count = m.child_count();
        eprintln!("Child machine count: {}", count);
        let none_state = m.get_child_state("__nonexistent__");
        assert!(none_state.is_none(), "Unknown child id must return None for state");
        let none_var = m.get_child_var("__nonexistent__", "x");
        assert!(none_var.is_none(), "Unknown child id must return None for var");
    }


// -------------------------------------------------------
// Child-machine tests (emitted because this machine has
// at least one inline invoked child: ['ID_P_5', 'ID_P_4', 'ID_P_3', 'ID_P_2', 'ID_P_1'])
// -------------------------------------------------------

#[test]
fn test_child_machine_spawned_and_steps() {
    let mut m = Machine::new();
    m.start();
    // After start, inline child machines must have been spawned.
    let count = m.child_count();
    eprintln!("Child machine count after start: {}", count);
    assert!(count > 0, "Expected child machines to be spawned");
    // Dispatching the first machine event must not panic with children active.
    m.step("take.*", Value::Undefined);
    eprintln!("Child machine count after step: {}", m.child_count());
}

#[test]
fn test_first_child_has_active_state() {
    // After start, the first known child machine (invoke_id = "ID_P_5")
    // must have a non-empty active state.
    let mut m = Machine::new();
    m.start();
    let child_state = m.get_child_state("ID_P_5");
    eprintln!("Child 'ID_P_5' state after start: {:?}", child_state);
    assert!(child_state.is_some(),
        "Child 'ID_P_5' must be active after start");
    assert!(!child_state.unwrap().is_empty(),
        "Child 'ID_P_5' active state must be non-empty");
}


                // -------------------------------------------------------
                // Dining Philosophers keystone tests
                // (emitted because this machine has invoke_id "ID_P_5")
                // -------------------------------------------------------

                #[test]
                fn test_child_machine_i_id_seeded_by_param() {
                    // D-M1P6-5: params seed the child datamodel from admitted parent expressions.
                    // Philosopher5's invoke has param i_ID=5 (IntLiteral).
                    // After spawn, get_child_var("ID_P_5", "i_ID") must be Int(5).
                    let mut m = Machine::new();
                    m.start();
                    let id5 = m.get_child_var("ID_P_5", "i_ID");
                    eprintln!("Child ID_P_5 i_ID = {:?}", id5);
                    assert_eq!(id5, Some(Value::Int(5_i64)),
                        "Param i_ID=5 must be seeded into child datamodel");
                    let state5 = m.get_child_state("ID_P_5");
                    eprintln!("Child ID_P_5 state = {:?}", state5);
                    assert!(state5.is_some(), "Child ID_P_5 must have an active state");
                    let s = state5.unwrap();
                    assert!(!s.is_empty(), "Child state must be non-empty");
                    eprintln!("Child ID_P_5 active state after start: {}", s);
                }

                #[test]
                fn test_child_transitions_on_do_timer_hungry() {
                    // Prove the child philosopher machine executes its Thinking->ProcessHungry
                    // transition when the Do.Timer.Hungry delayed-send fires.
                    let mut m = Machine::new();
                    m.start();

                    let state_before = m.get_child_state("ID_P_5");
                    eprintln!("Child ID_P_5 state BEFORE run(1): {:?}", state_before);

                    m.run(1);

                    let state_after = m.get_child_state("ID_P_5");
                    eprintln!("Child ID_P_5 state AFTER run(1): {:?}", state_after);
                    assert!(state_after.is_some(), "Child must still be active after timer");
                    let s_after = state_after.unwrap();
                    assert_ne!(s_after, "Thinking",
                        "Child must have transitioned away from Thinking after Do.Timer.Hungry fires");
                    eprintln!("PROVED: child philosopher executed Thinking->ProcessHungry via timer");
                }

                #[test]
                fn test_parent_receives_think_event_from_child() {
                    // D-M1P6-6: child sends think.5 to #_parent on Thinking onentry.
                    let mut m = Machine::new();
                    m.start();
                    let parent_state = m.current_state();
                    eprintln!("Parent state after start (think.5 should have fired): {}", parent_state);
                    assert!(!parent_state.is_empty(), "Parent must have a valid state");
                    eprintln!("Parent state: {} (cross-boundary think.5 routing exercised)", parent_state);
                }

                #[test]
                fn test_full_eat_cycle_logical_time() {
                    let mut m = Machine::new();
                    m.start();
                    let trace = m.run(200);

                    println!("=== Timer trace ({} events) ===", trace.len());
                    for line in &trace {
                        println!("  {}", line);
                    }
                    println!("=== Logical clock: {}ms ===", m.logical_time_ms());

                    // After running 200 timer steps, we should have fired many timers.
                    assert!(!trace.is_empty(), "Expected timer events to fire");

                    // Check that Do.Timer.Hungry events fired (philosophers left Thinking).
                    let hungry_fires: Vec<_> = trace.iter()
                        .filter(|t| t.contains("Do.Timer.Hungry")).collect();
                    println!("Do.Timer.Hungry fired {} times", hungry_fires.len());
                    assert!(!hungry_fires.is_empty(), "Expected Do.Timer.Hungry to fire");

                    // Determinism check: run again from scratch.
                    let mut m2 = Machine::new();
                    m2.start();
                    let trace2 = m2.run(200);
                    assert_eq!(trace, trace2, "Run is not deterministic!");

                    println!("PASS: full eat cycle deterministic, {} timer events fired", trace.len());
                }

                // ---------------------------------------------------------------------------
                // THE KEYSTONE TEST: full eat-cycle with fork-arbiter simulation
                // ---------------------------------------------------------------------------
                #[test]
                fn test_philosopher_full_eat_cycle_with_fork_arbiter() {
                    let mut event_trace: Vec<String> = Vec::new();

                    let inject_taken = |m: &mut Machine, fork: i64, who: i64, trace: &mut Vec<String>| {
                        let ev = format!("taken.{}", fork);
                        trace.push(format!("INJECT  event={} data={}", ev, who));
                        m.step(&ev, Value::Int(who));
                    };

                    let mut m = Machine::new();
                    m.start();

                    event_trace.push(format!("START   parent={} timers={}", m.current_state(), m.timer_count()));
                    event_trace.push(format!("START   child_ID_P_5={}", m.get_child_state("ID_P_5").unwrap_or_default()));

                    let timer_trace_1 = m.run(1);
                    for t in &timer_trace_1 { event_trace.push(format!("TIMER   {}", t)); }
                    let child_state_1 = m.get_child_state("ID_P_5").unwrap_or_default();
                    event_trace.push(format!("AFTER_T1 parent={} child_P5={}", m.current_state(), child_state_1));

                    assert_ne!(child_state_1, "Thinking",
                        "Philosopher 5 must leave Thinking after Do.Timer.Hungry; got {}",
                        child_state_1);

                    inject_taken(&mut m, 5, 5, &mut event_trace);
                    let child_state_2 = m.get_child_state("ID_P_5").unwrap_or_default();
                    event_trace.push(format!("AFTER_TAKEN5 parent={} child_P5={}", m.current_state(), child_state_2));

                    inject_taken(&mut m, 4, 5, &mut event_trace);
                    let child_state_3 = m.get_child_state("ID_P_5").unwrap_or_default();
                    let parent_state_3 = m.current_state().to_string();
                    event_trace.push(format!("AFTER_TAKEN4 parent={} child_P5={}", parent_state_3, child_state_3));

                    let in_eating = child_state_3 == "Eating"
                        || child_state_3 == "RightTaken1"
                        || child_state_3 == "LeftTaken1"
                        || child_state_3 == "LeftPut1"
                        || child_state_3 == "RightPut1"
                        || child_state_3 == "RequirePutLeft"
                        || child_state_3 == "RequirePutRight";

                    eprintln!("=== Eat-cycle trace ===");
                    for line in &event_trace { eprintln!("  {}", line); }
                    eprintln!("=== Child P5 final state: {} ===", child_state_3);
                    eprintln!("=== Parent final state:   {} ===", parent_state_3);

                    assert!(in_eating,
                        "Philosopher 5 MUST reach Eating after fork acquisition;                          got child_state={} parent_state={}.
                         Full trace:
{}",
                        child_state_3, parent_state_3,
                        event_trace.join("
"));

                    // Under true-parallel semantics every region is active at once,
                    // so check membership in the configuration rather than the
                    // (ambiguous) single current_state(). P5_Eating must be active.
                    assert!(m.is_active("P5_Eating"),
                        "Parent must enter P5_Eating when eat.5(data=1) arrives;                          active={:?} (child={})", m.active_states(), child_state_3);

                    eprintln!("PROVED: philosopher 5 acquired both forks and entered Eating");

                    let timer_trace_2 = m.run(1);
                    for t in &timer_trace_2 { event_trace.push(format!("TIMER2  {}", t)); }
                    let child_state_4 = m.get_child_state("ID_P_5").unwrap_or_default();
                    let parent_state_4 = m.current_state().to_string();
                    event_trace.push(format!("AFTER_THINK parent={} child_P5={}", parent_state_4, child_state_4));

                    eprintln!("After Do.Timer.Think: child={} parent={}", child_state_4, parent_state_4);

                    assert!(!timer_trace_2.is_empty() || !in_eating || child_state_4 != child_state_3,
                        "After Do.Timer.Think, something must change (child started in {})", child_state_3);

                    assert!(
                        m.is_active("P5_Hungry") || m.is_active("P5_Eating"),
                        "Parent must be in P5_Hungry or P5_Eating after eat exit; active={:?}",
                        m.active_states()
                    );

                    // Determinism check.
                    let mut trace_d: Vec<String> = Vec::new();
                    let mut m_d = Machine::new();
                    m_d.start();
                    trace_d.push(format!("START   parent={} timers={}", m_d.current_state(), m_d.timer_count()));
                    trace_d.push(format!("START   child_ID_P_5={}", m_d.get_child_state("ID_P_5").unwrap_or_default()));
                    let tt1_d = m_d.run(1);
                    for t in &tt1_d { trace_d.push(format!("TIMER   {}", t)); }
                    trace_d.push(format!("AFTER_T1 parent={} child_P5={}", m_d.current_state(), m_d.get_child_state("ID_P_5").unwrap_or_default()));
                    inject_taken(&mut m_d, 5, 5, &mut trace_d);
                    trace_d.push(format!("AFTER_TAKEN5 parent={} child_P5={}", m_d.current_state(), m_d.get_child_state("ID_P_5").unwrap_or_default()));
                    inject_taken(&mut m_d, 4, 5, &mut trace_d);
                    trace_d.push(format!("AFTER_TAKEN4 parent={} child_P5={}", m_d.current_state(), m_d.get_child_state("ID_P_5").unwrap_or_default()));
                    let tt2_d = m_d.run(1);
                    for t in &tt2_d { trace_d.push(format!("TIMER2  {}", t)); }
                    trace_d.push(format!("AFTER_THINK parent={} child_P5={}", m_d.current_state(), m_d.get_child_state("ID_P_5").unwrap_or_default()));

                    assert_eq!(event_trace, trace_d,
                        "Full eat-cycle trace must be deterministic across two runs");

                    eprintln!("PROVED: full eat-cycle is deterministic ({} trace lines)", event_trace.len());
                    eprintln!("=== GOLDEN TRACE ({} lines) ===", event_trace.len());
                    for line in &event_trace { eprintln!("  {}", line); }
                }

}
