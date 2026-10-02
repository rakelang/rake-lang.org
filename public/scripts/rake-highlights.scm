; Rake canonical syntax highlighting

(identifier) @variable

[
  (line_comment)
  (block_comment)
] @comment

[
  "pack"
  "record"
  "union"
  "scratch"
  "rake"
  "run"
  "slow"
  "extern"
  "state"
  "embed"
  "const"
  "let"
  "return"
  "yield"
] @keyword

[
  "tine"
  "means"
  "through"
  "else"
  "into"
  "sweep"
  "for"
  "in"
  "using"
  "up"
  "to"
  "from"
  "by"
  "if"
  "then"
  "while"
  "repeat"
  "unchecked"
  "ptr"
  "mut"
] @keyword

(break_statement) @keyword
(continue_statement) @keyword

[
  "stack"
  "pack"
] @keyword

(storage_type) @type.builtin
(rack_type) @type.builtin
(mask_type) @type.builtin
(void_type) @type.builtin
(type_identifier) @type
(record_definition name: (type_identifier) @type)
(union_definition name: (type_identifier) @type)
(record_expression record: (type_identifier) @type)
(stack_expression schema: (type_identifier) @type)

(pack_definition name: (type_identifier) @type)
(scratch_definition name: (identifier) @function)
(rake_definition name: (identifier) @function)
(run_definition name: (identifier) @function)
(slow_definition name: (identifier) @function)
(extern_definition name: (identifier) @function)
(conversion_expression kind: _ @function.builtin)
(call_expression function: (identifier) @function)
(call_expression function: "fma" @function.builtin)
(shuffle_expression "shuffle" @function)
(static_move_expression function: _ @function)

(field_group name: (identifier) @property)
(record_field_group name: (identifier) @property)
(field_initializer field: (identifier) @property)
(field_expression field: (identifier) @property)

(value_parameter name: (identifier) @variable.parameter)
(scalar_parameter name: (scalar_name) @variable.parameter)
(scalar_variable name: (scalar_name) @variable)
(uniform_binding name: (scalar_name) @variable)

(let_statement name: (identifier) @variable)
(fused_binding name: (identifier) @variable)
(mutable_binding name: (identifier) @variable)
(assignment_statement target: (primary_expression (identifier) @variable))
(through_statement name: (identifier) @variable)
(traversal_statement binding: (identifier) @variable)

; Rake marks one idea with each symbol: a tine is a mask, angle brackets a
; uniform, and the bars and arrows of a fused binding or sweep a data flow.
(tine "#" @tag (tine_name) @tag)
(scalar_parameter "<" @constant ">" @constant)
(scalar_expression "<" @constant ">" @constant) @constant

(fused_binding "|" @operator.flow "<|" @operator.flow)
(sweep_arm "|" @operator.flow "=>" @operator.flow)
(sweep_arm selector: "_" @constant.builtin)

[
  "->"
  ":="
  "<-"
  "="
  "+"
  "-"
  "*"
  "/"
  "%"
  "<"
  "<="
  ">"
  ">="
  "!="
  "and"
  "or"
  "not"
  "gaps"
] @operator

(integer_literal) @number
(string_literal) @string
(float_literal) @number
(boolean_literal) @boolean
(lanes) @constant.builtin
(lane_index) @constant.builtin

[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

[
  ","
  ";"
  ":"
  "."
] @punctuation.delimiter
