#!/usr/bin/env python3
"""Writes the code grammars that Keeper adds to keybr.com's.

The output goes to assets/code/keeper, which the keybr.com extraction tool
leaves alone: one lang_<id>.json of rules per language, _names.json with
names shared by these languages, and syntaxes.json, the list that Keeper
loads after keybr's.

Rules use keybr's grammar format. A string is literal text, and these
combine them:

    {"seq": [a, b]}                       a, then b
    {"alt": [a, b]}                       a or b, picked at random
    {"f": 0.5, "opt": a}                  a, half of the time
    {"ref": "name"}                       the rule called name
    {"flag": "f", "inv": false, "cond": a}  a, if the lesson option f is on
    {"cls": "keyword", "span": a}         a, highlighted as a keyword

A space between items may become a line break in real code; lessons type
it as one space. Rules missing from a language's file come from keybr's
shared files: _keywords (kw_if, kw_return...), _ident (snake_case names),
_literals (numeric_literal, generic_string_content...) and _comments
(comment_text). The lesson options are the flags comments, defs, numbers,
strings and types.

After changing a grammar, run this from the repository root and check the
lessons with `cargo test`:

    python3 tools/code-grammars.py
"""

import json
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "assets" / "code" / "keeper"
SHARED = ["_keywords", "_ident", "_literals", "_comments"]
SP = " "


def seq(*items):
    return {"seq": list(items)}


def alt(*items):
    return {"alt": list(items)}


def opt(item, f=0.5):
    return {"f": f, "opt": item}


def ref(name):
    return {"ref": name}


def flag(name, item):
    return {"flag": name, "inv": False, "cond": item}


def kw(word):
    return {"cls": "keyword", "span": word}


def string(item):
    return {"cls": "string", "span": item}


def number(item):
    return {"cls": "number", "span": item}


def comment(item):
    return {"cls": "comment", "span": item}


def binary(operand, operators, rest):
    """`operand op rest`, recursing on the right so that text comes first."""
    return seq(ref(operand), SP, alt(*operators), SP, ref(rest))


# Names shared by Keeper's languages. keybr's generic_function_name has a
# space after get and set ("get file_size"), so snake_case languages use
# snake_function_name instead.
NAMES = {
    "camel_variable_name": alt(
        "count", "index", "value", "result", "items", "user", "users", "config",
        "buffer", "total", "name", "data", "key", "size", "width", "height",
        "offset", "request", "response", "token", "session", "position",
        seq(
            alt("max", "min", "next", "last", "first", "current", "default", "new", "old"),
            alt("Value", "Index", "Count", "Item", "User", "Size", "Width", "Name", "Key", "Id"),
        ),
        seq(alt("is", "has", "should"), alt("Valid", "Empty", "Ready", "Enabled", "Visible", "Loaded")),
    ),
    "camel_function_name": seq(
        alt(
            "get", "set", "load", "save", "update", "build", "parse", "find", "create",
            "remove", "handle", "fetch", "render", "compute", "validate", "format", "reset",
        ),
        alt(
            "User", "Data", "Config", "Item", "Value", "Items", "Name", "Request",
            "Response", "Result", "Index", "Cache", "Session", "Token", "Layout",
        ),
    ),
    "snake_function_name": seq(
        alt(
            "get", "set", "load", "save", "update", "build", "parse", "find", "create",
            "remove", "handle", "fetch", "render", "validate", "format", "reset", "process",
            "send", "read", "write",
        ),
        opt(seq("_", alt(
            "user", "data", "config", "item", "items", "value", "name", "request",
            "response", "result", "cache", "session", "token", "file", "line",
        )), 0.7),
    ),
}

LANGUAGES = []


def language(id, name, rules):
    """Registers a grammar; its `start` rule makes one statement."""
    for rule, body in rules.items():
        # An alternative that refers to this rule would still be picked
        # with the flag off, and print nothing: flag where it is used.
        assert "flag" not in body, f"{id}: move the flag of {rule} to where it is used"
    LANGUAGES.append((id, name, rules))


# ---------------------------------------------------------------------------
# Kotlin

language("kotlin", "Kotlin", rules={
    "start": ref("kt_statement"),
    "kt_statement": alt(
        ref("kt_property"),
        ref("kt_property"),
        flag("defs", ref("kt_function")),
        flag("defs", ref("kt_class")),
        ref("kt_if"),
        ref("kt_for"),
        ref("kt_when"),
        ref("kt_call"),
        ref("kt_lambda_call"),
        ref("kt_return"),
        flag("comments", ref("kt_comment")),
    ),
    "kt_property": seq(
        alt(kw("val"), kw("val"), kw("var")), SP, ref("kt_name"),
        flag("types", seq(":", SP, ref("kt_type"))), SP, "=", SP, ref("kt_expression"),
    ),
    "kt_function": seq(
        opt(seq(alt(kw("private"), kw("override"), kw("suspend"), kw("inline")), SP), 0.3),
        kw("fun"), SP, ref("kt_function_name"), "(", opt(ref("kt_params")), ")",
        flag("types", seq(":", SP, ref("kt_type"))), SP,
        alt(
            seq("=", SP, ref("kt_expression")),
            seq("{", SP, ref("kt_return"), SP, "}"),
        ),
    ),
    "kt_params": seq(ref("kt_param"), opt(seq(",", SP, ref("kt_param")))),
    "kt_param": seq(
        ref("kt_name"), ":", SP, ref("kt_type"),
        opt(seq(SP, "=", SP, ref("kt_literal")), 0.25),
    ),
    "kt_class": alt(
        seq(
            kw("data"), SP, kw("class"), SP, ref("kt_type_name"), "(",
            ref("kt_class_param"), opt(seq(",", SP, ref("kt_class_param"))), ")",
        ),
        seq(
            kw("class"), SP, ref("kt_type_name"), opt(seq("(", ref("kt_class_param"), ")")),
            SP, ":", SP, ref("kt_type_name"), "()",
        ),
        seq(kw("object"), SP, ref("kt_type_name"), SP, "{", SP, ref("kt_property"), SP, "}"),
        seq(
            kw("enum"), SP, kw("class"), SP, ref("kt_type_name"), SP, "{", SP,
            alt("IDLE, RUNNING, DONE", "LEFT, RIGHT, UP, DOWN", "LOW, MEDIUM, HIGH"), SP, "}",
        ),
        seq(kw("interface"), SP, ref("kt_type_name"), SP, "{", SP, kw("fun"), SP,
            ref("kt_function_name"), "()", SP, "}"),
    ),
    "kt_class_param": seq(alt(kw("val"), kw("var")), SP, ref("kt_name"), ":", SP, ref("kt_type")),
    "kt_if": seq(
        kw("if"), SP, "(", ref("kt_condition"), ")", SP, "{", SP, ref("kt_call"), SP, "}",
        opt(seq(SP, kw("else"), SP, "{", SP, ref("kt_call"), SP, "}"), 0.4),
    ),
    "kt_for": seq(
        kw("for"), SP, "(", ref("kt_name"), SP, kw("in"), SP,
        alt(
            ref("kt_name"),
            flag("numbers", seq(ref("kt_int"), "..", ref("kt_int"))),
            flag("numbers", seq(ref("kt_int"), SP, kw("until"), SP, ref("kt_name"), ".size")),
            seq(ref("kt_name"), ".indices"),
        ),
        ")", SP, "{", SP, ref("kt_call"), SP, "}",
    ),
    "kt_when": seq(
        kw("when"), SP, "(", ref("kt_name"), ")", SP, "{", SP,
        alt(ref("kt_literal"), seq(kw("is"), SP, ref("kt_type_name"))), SP, "->", SP,
        ref("kt_expression"), SP, kw("else"), SP, "->", SP, ref("kt_expression"), SP, "}",
    ),
    "kt_call": seq(
        opt(seq(ref("kt_name"), alt(".", "?.")), 0.6), ref("kt_function_name"), "(",
        opt(seq(ref("kt_argument"), opt(seq(",", SP, ref("kt_argument"))))), ")",
    ),
    "kt_argument": alt(ref("kt_name"), ref("kt_literal"), seq(ref("kt_name"), SP, "=", SP, ref("kt_literal"))),
    "kt_lambda_call": seq(
        ref("kt_name"), ".",
        alt("map", "filter", "forEach", "any", "sumOf", "sortedBy", "firstOrNull", "count"),
        SP, "{", SP, ref("kt_lambda_body"), SP, "}",
    ),
    "kt_lambda_body": alt(
        seq("it", SP, alt("*", "+", "-"), SP, ref("kt_int_or_name")),
        seq("it", ".", ref("kt_name")),
        seq("it", SP, alt(">", "<", "==", "!="), SP, ref("kt_literal")),
        seq(ref("kt_name"), SP, "->", SP, ref("kt_name"), ".", ref("kt_name")),
    ),
    "kt_return": seq(kw("return"), SP, ref("kt_expression")),
    "kt_condition": alt(
        seq(ref("kt_name"), SP, alt("==", "!=", ">", "<", ">=", "<="), SP, ref("kt_literal")),
        seq(ref("kt_name"), SP, alt(kw("is"), seq("!", kw("is"))), SP, ref("kt_type_name")),
        seq(ref("kt_name"), ".", alt("isEmpty()", "isNotEmpty()", "isBlank()")),
        seq(ref("kt_name"), SP, "!=", SP, kw("null")),
    ),
    "kt_expression": alt(
        ref("kt_literal"),
        ref("kt_name"),
        ref("kt_call"),
        binary("kt_name", ["+", "-", "*", "/", "%"], "kt_expression"),
        seq(ref("kt_name"), SP, "?:", SP, ref("kt_literal")),
        seq("listOf(", ref("kt_literal"), ",", SP, ref("kt_literal"), ")"),
        seq("mapOf(", ref("kt_name"), SP, kw("to"), SP, ref("kt_literal"), ")"),
        seq(ref("kt_type_name"), "(", opt(ref("kt_argument")), ")"),
        flag("strings", ref("kt_template")),
    ),
    "kt_literal": alt(
        flag("numbers", ref("kt_number")),
        flag("strings", ref("kt_string")),
        kw("true"), kw("false"), kw("null"),
    ),
    "kt_number": number(ref("numeric_literal")),
    "kt_int": number(ref("integer_dec_literal")),
    "kt_int_or_name": alt(flag("numbers", ref("kt_int")), ref("kt_name")),
    "kt_string": string(seq('"', ref("generic_string_content"), '"')),
    "kt_template": string(seq(
        '"', alt("Hello,", "Total:", "Loaded", "Count:"), SP, "$", ref("kt_name"), '"',
    )),
    "kt_type": alt(
        ref("kt_basic_type"),
        ref("kt_basic_type"),
        seq("List<", ref("kt_basic_type"), ">"),
        seq("Map<String,", SP, ref("kt_basic_type"), ">"),
        seq(ref("kt_basic_type"), "?"),
    ),
    "kt_basic_type": alt("Int", "String", "Boolean", "Double", "Long", ref("kt_type_name")),
    "kt_type_name": ref("generic_class_name"),
    "kt_name": ref("camel_variable_name"),
    "kt_function_name": ref("camel_function_name"),
    "kt_comment": comment(seq("// ", ref("comment_text"))),
})


# ---------------------------------------------------------------------------
# Swift

language("swift", "Swift", rules={
    "start": ref("sw_statement"),
    "sw_statement": alt(
        ref("sw_binding"),
        ref("sw_binding"),
        flag("defs", ref("sw_function")),
        flag("defs", ref("sw_type_declaration")),
        ref("sw_if"),
        ref("sw_guard"),
        ref("sw_for"),
        ref("sw_switch"),
        ref("sw_call"),
        ref("sw_return"),
        flag("comments", ref("sw_comment")),
    ),
    "sw_binding": seq(
        alt(kw("let"), kw("let"), kw("var")), SP, ref("sw_name"),
        flag("types", seq(":", SP, ref("sw_type"))), SP, "=", SP, ref("sw_expression"),
    ),
    "sw_function": seq(
        opt(seq(alt(kw("private"), kw("public"), kw("static"), kw("override"), kw("mutating")), SP), 0.3),
        kw("func"), SP, ref("sw_function_name"), "(", opt(ref("sw_params")), ")",
        opt(seq(SP, alt(kw("async"), kw("throws"), seq(kw("async"), SP, kw("throws")))), 0.3),
        opt(seq(SP, "->", SP, ref("sw_type")), 0.7),
        SP, "{", SP, ref("sw_return"), SP, "}",
    ),
    "sw_params": seq(ref("sw_param"), opt(seq(",", SP, ref("sw_param")))),
    "sw_param": seq(
        opt(seq(alt("_", "with", "for", "at", "from"), SP), 0.4),
        ref("sw_name"), ":", SP, ref("sw_type"),
        opt(seq(SP, "=", SP, ref("sw_literal")), 0.2),
    ),
    "sw_type_declaration": alt(
        seq(
            kw("struct"), SP, ref("sw_type_name"),
            opt(seq(":", SP, alt("Codable", "Equatable", "Hashable", "Identifiable", "View"))),
            SP, "{", SP, ref("sw_member"), opt(seq(SP, ref("sw_member"))), SP, "}",
        ),
        seq(
            opt(seq(kw("final"), SP), 0.3), kw("class"), SP, ref("sw_type_name"),
            opt(seq(":", SP, ref("sw_type_name"))), SP, "{", SP, ref("sw_member"), SP, "}",
        ),
        seq(
            kw("enum"), SP, ref("sw_type_name"), SP, "{", SP, kw("case"), SP,
            alt("idle, loading, loaded, failed", "north, south, east, west", "small, medium, large"),
            SP, "}",
        ),
        seq(kw("protocol"), SP, ref("sw_type_name"), SP, "{", SP, kw("func"), SP,
            ref("sw_function_name"), "()", SP, "}"),
        seq(kw("extension"), SP, ref("sw_type_name"), ":", SP,
            alt("Equatable", "CustomStringConvertible", "Sendable"), SP, "{", SP, "}"),
    ),
    "sw_member": seq(alt(kw("let"), kw("var")), SP, ref("sw_name"), ":", SP, ref("sw_type")),
    "sw_if": seq(
        kw("if"), SP,
        alt(
            ref("sw_condition"),
            seq(kw("let"), SP, ref("sw_name"), SP, "=", SP, ref("sw_optional")),
            seq(kw("let"), SP, ref("sw_name")),
        ),
        SP, "{", SP, ref("sw_call"), SP, "}",
        opt(seq(SP, kw("else"), SP, "{", SP, ref("sw_call"), SP, "}"), 0.4),
    ),
    "sw_guard": seq(
        kw("guard"), SP, kw("let"), SP, ref("sw_name"), SP, "=", SP, ref("sw_optional"), SP,
        kw("else"), SP, "{", SP, kw("return"), opt(seq(SP, kw("nil")), 0.3), SP, "}",
    ),
    "sw_optional": alt(
        seq(ref("sw_name"), "?.", ref("sw_name")),
        seq(ref("sw_name"), ".first"),
        seq(ref("sw_name"), ".last"),
        seq(ref("sw_name"), "[", ref("sw_name"), "]"),
        seq(ref("sw_type_name"), "(", ref("sw_name"), ")"),
    ),
    "sw_for": seq(
        kw("for"), SP, ref("sw_name"), SP, kw("in"), SP,
        alt(
            ref("sw_name"),
            flag("numbers", seq(ref("sw_int"), "..<", ref("sw_int"))),
            flag("numbers", seq(ref("sw_int"), "..<", ref("sw_name"), ".count")),
            seq(ref("sw_name"), ".indices"),
        ),
        opt(seq(SP, kw("where"), SP, ref("sw_condition")), 0.25),
        SP, "{", SP, ref("sw_call"), SP, "}",
    ),
    "sw_switch": seq(
        kw("switch"), SP, ref("sw_name"), SP, "{", SP, kw("case"), SP, ".",
        alt("idle", "loading", "success", "failure", "none"), ":", SP, ref("sw_call"), SP,
        kw("default"), ":", SP, kw("break"), SP, "}",
    ),
    "sw_call": alt(
        seq(opt(seq(ref("sw_name"), alt(".", "?.")), 0.5), ref("sw_function_name"), "(", opt(ref("sw_args")), ")"),
        seq(alt(kw("try"), kw("await"), seq(kw("try"), SP, kw("await"))), SP, ref("sw_function_name"), "()"),
        seq("print(", ref("sw_simple"), ")"),
        seq(
            ref("sw_name"), ".", alt("map", "filter", "forEach", "sorted", "compactMap", "contains"),
            SP, "{", SP, ref("sw_closure"), SP, "}",
        ),
    ),
    "sw_args": seq(ref("sw_arg"), opt(seq(",", SP, ref("sw_arg")))),
    "sw_arg": seq(opt(seq(ref("sw_name"), ":", SP), 0.5), ref("sw_simple")),
    "sw_closure": alt(
        seq("$0", SP, alt("*", "+", "-"), SP, ref("sw_int_or_name")),
        seq("$0", ".", ref("sw_name")),
        seq("$0", SP, alt(">", "<", "==", "!="), SP, ref("sw_literal")),
        seq(ref("sw_name"), SP, kw("in"), SP, ref("sw_name"), ".", ref("sw_name")),
    ),
    "sw_return": seq(kw("return"), SP, ref("sw_expression")),
    "sw_condition": alt(
        seq(ref("sw_name"), SP, alt("==", "!=", ">", "<", ">=", "<="), SP, ref("sw_literal")),
        seq(ref("sw_name"), ".isEmpty"),
        seq("!", ref("sw_name"), ".isEmpty"),
        seq(ref("sw_name"), SP, "!=", SP, kw("nil")),
    ),
    "sw_expression": alt(
        ref("sw_literal"),
        ref("sw_name"),
        ref("sw_call"),
        binary("sw_name", ["+", "-", "*", "/", "%"], "sw_expression"),
        seq(ref("sw_name"), SP, "??", SP, ref("sw_literal")),
        seq("[", ref("sw_literal"), ",", SP, ref("sw_literal"), "]"),
        seq(ref("sw_type_name"), "(", opt(ref("sw_args")), ")"),
        flag("strings", ref("sw_interpolation")),
    ),
    "sw_simple": alt(ref("sw_literal"), ref("sw_name")),
    "sw_literal": alt(
        flag("numbers", ref("sw_number")),
        flag("strings", ref("sw_string")),
        kw("true"), kw("false"), kw("nil"),
    ),
    "sw_number": number(ref("numeric_literal")),
    "sw_int": number(ref("integer_dec_literal")),
    "sw_int_or_name": alt(flag("numbers", ref("sw_int")), ref("sw_name")),
    "sw_string": string(seq('"', ref("generic_string_content"), '"')),
    "sw_interpolation": string(seq(
        '"', alt("Hello,", "Total:", "Loaded", "Count:"), SP, "\\(", ref("sw_name"), ")", '"',
    )),
    "sw_type": alt(
        ref("sw_basic_type"),
        ref("sw_basic_type"),
        seq("[", ref("sw_basic_type"), "]"),
        seq("[String:", SP, ref("sw_basic_type"), "]"),
        seq(ref("sw_basic_type"), "?"),
    ),
    "sw_basic_type": alt("Int", "String", "Bool", "Double", ref("sw_type_name")),
    "sw_type_name": ref("generic_class_name"),
    "sw_name": ref("camel_variable_name"),
    "sw_function_name": ref("camel_function_name"),
    "sw_comment": comment(seq("// ", ref("comment_text"))),
})


# ---------------------------------------------------------------------------
# Dart

language("dart", "Dart", rules={
    "start": ref("dt_statement"),
    "dt_statement": alt(
        ref("dt_variable"),
        ref("dt_variable"),
        flag("defs", ref("dt_function")),
        flag("defs", ref("dt_class")),
        ref("dt_if"),
        ref("dt_for"),
        ref("dt_call_statement"),
        ref("dt_return"),
        flag("comments", ref("dt_comment")),
    ),
    "dt_variable": seq(
        alt(kw("final"), kw("final"), kw("var"), kw("const"), flag("types", ref("dt_type"))),
        SP, ref("dt_name"), SP, "=", SP, ref("dt_expression"), ";",
    ),
    "dt_function": seq(
        alt(flag("types", ref("dt_type")), "void", seq("Future<", ref("dt_basic_type"), ">")),
        SP, ref("dt_function_name"), "(", opt(ref("dt_params")), ")",
        opt(seq(SP, kw("async")), 0.3), SP,
        alt(
            seq("=>", SP, ref("dt_expression"), ";"),
            seq("{", SP, ref("dt_return"), SP, "}"),
        ),
    ),
    "dt_params": alt(
        seq(ref("dt_param"), opt(seq(",", SP, ref("dt_param")))),
        seq("{", kw("required"), SP, ref("dt_param"), opt(seq(",", SP, ref("dt_param"))), "}"),
    ),
    "dt_param": seq(ref("dt_type"), SP, ref("dt_name")),
    "dt_class": alt(
        seq(
            opt(seq(kw("abstract"), SP), 0.2), kw("class"), SP, ref("dt_type_name"),
            opt(seq(SP, alt(kw("extends"), kw("implements"), kw("with")), SP, ref("dt_type_name"))),
            SP, "{", SP, kw("final"), SP, ref("dt_type"), SP, ref("dt_name"), ";", SP, "}",
        ),
        seq(
            kw("enum"), SP, ref("dt_type_name"), SP, "{", SP,
            alt("idle, loading, done", "small, medium, large", "north, south, east, west"), SP, "}",
        ),
        seq(kw("mixin"), SP, ref("dt_type_name"), SP, "{", SP, "}"),
        seq(kw("extension"), SP, ref("dt_type_name"), "Extension", SP, kw("on"), SP,
            ref("dt_type"), SP, "{", SP, "}"),
    ),
    "dt_if": seq(
        kw("if"), SP, "(", ref("dt_condition"), ")", SP, "{", SP, ref("dt_call"), ";", SP, "}",
        opt(seq(SP, kw("else"), SP, "{", SP, ref("dt_call"), ";", SP, "}"), 0.4),
    ),
    "dt_for": alt(
        seq(kw("for"), SP, "(", kw("final"), SP, ref("dt_name"), SP, kw("in"), SP, ref("dt_name"),
            ")", SP, "{", SP, ref("dt_call"), ";", SP, "}"),
        flag("numbers", seq(
            kw("for"), SP, "(", kw("var"), SP, "i", SP, "=", SP, number("0"), ";", SP, "i", SP, "<",
            SP, ref("dt_name"), ".length;", SP, "i++)", SP, "{", SP, ref("dt_call"), ";", SP, "}",
        )),
    ),
    "dt_call_statement": seq(alt(ref("dt_call"), seq(kw("await"), SP, ref("dt_call")), ref("dt_chain")), ";"),
    "dt_call": alt(
        seq(opt(seq(ref("dt_name"), alt(".", "?.")), 0.6), ref("dt_function_name"), "(", opt(ref("dt_args")), ")"),
        seq("print(", ref("dt_simple"), ")"),
        seq("setState(()", SP, "=>", SP, ref("dt_name"), SP, "=", SP, ref("dt_simple"), ")"),
    ),
    "dt_chain": seq(
        ref("dt_name"), ".", alt("where", "map", "any", "every"), "((", ref("dt_name"), ")",
        SP, "=>", SP, ref("dt_lambda_body"), ")",
        opt(seq(".", alt("toList()", "toSet()", "first", "isEmpty")), 0.6),
    ),
    "dt_lambda_body": alt(
        seq(ref("dt_name"), ".", ref("dt_name")),
        seq(ref("dt_name"), SP, alt(">", "<", "==", "!="), SP, ref("dt_literal")),
        seq(ref("dt_name"), SP, alt("*", "+"), SP, ref("dt_int_or_name")),
    ),
    "dt_args": seq(ref("dt_arg"), opt(seq(",", SP, ref("dt_arg")))),
    "dt_arg": alt(ref("dt_simple"), seq(ref("dt_name"), ":", SP, ref("dt_simple"))),
    "dt_return": seq(kw("return"), SP, ref("dt_expression"), ";"),
    "dt_condition": alt(
        seq(ref("dt_name"), SP, alt("==", "!=", ">", "<", ">=", "<="), SP, ref("dt_literal")),
        seq(ref("dt_name"), ".", alt("isEmpty", "isNotEmpty")),
        seq(ref("dt_name"), SP, kw("is"), SP, ref("dt_type_name")),
        seq(ref("dt_name"), SP, "!=", SP, kw("null")),
    ),
    "dt_expression": alt(
        ref("dt_literal"),
        ref("dt_name"),
        ref("dt_call"),
        binary("dt_name", ["+", "-", "*", "/", "~/", "%"], "dt_expression"),
        seq(ref("dt_name"), SP, "??", SP, ref("dt_literal")),
        seq("[", ref("dt_literal"), ",", SP, ref("dt_literal"), "]"),
        seq("{", alt(flag("strings", ref("dt_string")), ref("dt_name")), ":", SP, ref("dt_literal"), "}"),
        seq(ref("dt_type_name"), "(", opt(ref("dt_args")), ")"),
        flag("strings", ref("dt_interpolation")),
    ),
    "dt_simple": alt(ref("dt_literal"), ref("dt_name")),
    "dt_literal": alt(
        flag("numbers", ref("dt_number")),
        flag("strings", ref("dt_string")),
        kw("true"), kw("false"), kw("null"),
    ),
    "dt_number": number(ref("numeric_literal")),
    "dt_int_or_name": alt(flag("numbers", number(ref("integer_dec_literal"))), ref("dt_name")),
    "dt_string": string(alt(
        seq("'", ref("generic_string_content"), "'"),
        seq('"', ref("generic_string_content"), '"'),
    )),
    "dt_interpolation": string(seq(
        "'", alt("Hello,", "Total:", "Loaded", "Count:"), SP, "$", ref("dt_name"), "'",
    )),
    "dt_type": alt(
        ref("dt_basic_type"),
        ref("dt_basic_type"),
        seq("List<", ref("dt_basic_type"), ">"),
        seq("Map<String,", SP, ref("dt_basic_type"), ">"),
        seq(ref("dt_basic_type"), "?"),
    ),
    "dt_basic_type": alt("int", "String", "bool", "double", "num", ref("dt_type_name")),
    "dt_type_name": ref("generic_class_name"),
    "dt_name": ref("camel_variable_name"),
    "dt_function_name": ref("camel_function_name"),
    "dt_comment": comment(alt(seq("// ", ref("comment_text")), seq("/// ", ref("comment_text")))),
})


# ---------------------------------------------------------------------------
# Ruby

language("ruby", "Ruby", rules={
    "start": ref("rb_statement"),
    "rb_statement": alt(
        ref("rb_assign"),
        ref("rb_assign"),
        flag("defs", ref("rb_def")),
        flag("defs", ref("rb_class")),
        ref("rb_call"),
        ref("rb_block"),
        ref("rb_if"),
        ref("rb_modifier"),
        ref("rb_attr"),
        flag("strings", ref("rb_require")),
        flag("comments", ref("rb_comment")),
    ),
    "rb_assign": alt(
        seq(alt(ref("rb_name"), seq("@", ref("rb_name"))), SP, alt("=", "=", "||=", "+="), SP, ref("rb_expression")),
        seq(ref("rb_constant"), SP, "=", SP, ref("rb_expression")),
    ),
    "rb_constant": alt("MAX_SIZE", "DEFAULT_TIMEOUT", "VERSION", "LIMIT", "API_URL", "RETRIES"),
    "rb_def": alt(
        seq(
            kw("def"), SP, opt(seq(kw("self"), "."), 0.2), ref("rb_method_name"),
            "(", ref("rb_params"), ")", SP, ref("rb_body"), SP, kw("end"),
        ),
        seq(kw("def"), SP, ref("rb_method_name"), SP, "=", SP, ref("rb_expression")),
    ),
    "rb_method_name": alt(
        ref("snake_function_name"),
        ref("snake_function_name"),
        seq(ref("snake_function_name"), "?"),
        seq(ref("snake_function_name"), "!"),
        "to_s", "initialize", "each", "call",
    ),
    "rb_params": seq(
        ref("rb_name"), opt(seq(",", SP, ref("rb_name"))),
        opt(seq(",", SP, ref("rb_name"), ":", SP, ref("rb_literal")), 0.3),
    ),
    "rb_body": alt(
        ref("rb_expression"),
        ref("rb_call"),
        seq(ref("rb_name"), ".", ref("rb_method_name")),
    ),
    "rb_class": alt(
        seq(kw("class"), SP, ref("rb_class_name"), opt(seq(SP, "<", SP, ref("rb_class_name"))),
            SP, ref("rb_attr"), SP, kw("end")),
        seq(kw("module"), SP, ref("rb_class_name"), SP, kw("end")),
        seq(kw("class"), SP, ref("rb_class_name"), "Error", SP, "<", SP, "StandardError;", SP, kw("end")),
        seq(kw("include"), SP, alt("Comparable", "Enumerable")),
    ),
    "rb_attr": seq(
        alt(kw("attr_reader"), kw("attr_accessor"), kw("attr_writer")), SP,
        ref("rb_symbol"), opt(seq(",", SP, ref("rb_symbol"))),
    ),
    "rb_require": seq(
        alt(kw("require"), kw("require_relative")), SP,
        string(seq('"', alt("json", "set", "time", "net/http", "yaml", "logger", "./config"), '"')),
    ),
    "rb_call": alt(
        seq(alt("puts", "p", kw("raise"), "logger.info"), SP, ref("rb_expression")),
        seq(ref("rb_name"), ".", ref("rb_method_name"), opt(seq("(", ref("rb_args"), ")"))),
        seq(ref("rb_class_name"), ".", alt("new", "find", "create", "call"), "(", opt(ref("rb_args")), ")"),
    ),
    "rb_args": seq(ref("rb_arg"), opt(seq(",", SP, ref("rb_arg")))),
    "rb_arg": alt(ref("rb_name"), ref("rb_literal"), seq(ref("rb_name"), ":", SP, ref("rb_literal"))),
    "rb_block": alt(
        seq(
            ref("rb_name"), ".", alt("each", "map", "select", "reject", "each_with_index", "group_by"),
            SP, kw("do"), SP, "|", ref("rb_name"), "|", SP, ref("rb_call"), SP, kw("end"),
        ),
        seq(
            ref("rb_name"), ".", alt("map", "select", "sum", "sort_by", "find", "count"),
            SP, "{", SP, "|", ref("rb_name"), "|", SP, ref("rb_name"), ".", ref("rb_method_name"), SP, "}",
        ),
        seq(ref("rb_name"), ".", alt("map", "select", "each", "sum"), "(&", ref("rb_symbol"), ")"),
        flag("numbers", seq(ref("rb_int"), ".times", SP, "{", SP, "|i|", SP, ref("rb_call"), SP, "}")),
    ),
    "rb_if": seq(
        alt(kw("if"), kw("unless")), SP, ref("rb_condition"), SP, kw("then"), SP, ref("rb_call"),
        opt(seq(SP, kw("else"), SP, ref("rb_call")), 0.4), SP, kw("end"),
    ),
    "rb_modifier": seq(
        alt(ref("rb_call"), seq(kw("return"), SP, ref("rb_expression")), kw("next"), kw("break")),
        SP, alt(kw("if"), kw("unless")), SP, ref("rb_condition"),
    ),
    "rb_condition": alt(
        seq(ref("rb_name"), SP, alt("==", "!=", ">", "<", ">=", "<="), SP, ref("rb_literal")),
        seq(ref("rb_name"), ".", alt("empty?", "nil?", "any?", "zero?", "valid?", "include?(x)")),
        seq(ref("rb_name"), SP, "&&", SP, ref("rb_name"), ".", alt("active?", "present?")),
    ),
    "rb_expression": alt(
        ref("rb_literal"),
        ref("rb_name"),
        binary("rb_name", ["+", "-", "*", "/", "%", "<<"], "rb_expression"),
        seq("[", ref("rb_literal"), ",", SP, ref("rb_literal"), "]"),
        seq("{", SP, ref("rb_name"), ":", SP, ref("rb_literal"), SP, "}"),
        seq(ref("rb_name"), SP, "||", SP, ref("rb_literal")),
        seq(ref("rb_class_name"), ".new"),
        flag("strings", ref("rb_interpolation")),
    ),
    "rb_literal": alt(
        flag("numbers", ref("rb_number")),
        flag("strings", ref("rb_string")),
        ref("rb_symbol"),
        kw("nil"), kw("true"), kw("false"),
    ),
    "rb_symbol": string(seq(":", ref("rb_name"))),
    "rb_number": number(ref("numeric_literal")),
    "rb_int": number(ref("integer_dec_literal")),
    "rb_string": string(alt(
        seq('"', ref("generic_string_content"), '"'),
        seq("'", ref("generic_string_content"), "'"),
    )),
    "rb_interpolation": string(seq(
        '"', alt("Hello,", "Total:", "Saved", "User"), SP, "#{", ref("rb_name"), "}", '"',
    )),
    "rb_name": ref("generic_variable_name"),
    "rb_class_name": ref("generic_class_name"),
    "rb_comment": comment(seq("# ", ref("comment_text"))),
})


# ---------------------------------------------------------------------------
# Elixir

language("elixir", "Elixir", rules={
    "start": ref("ex_statement"),
    "ex_statement": alt(
        ref("ex_match"),
        ref("ex_match"),
        flag("defs", ref("ex_def")),
        flag("defs", ref("ex_module")),
        ref("ex_pipeline"),
        ref("ex_case"),
        ref("ex_if"),
        ref("ex_call"),
        flag("types", ref("ex_spec")),
        flag("comments", ref("ex_comment")),
    ),
    "ex_match": seq(
        alt(ref("ex_name"), ref("ex_tuple_pattern"), seq("%{", ref("ex_name"), ":", SP, ref("ex_name"), "}")),
        SP, "=", SP, ref("ex_expression"),
    ),
    "ex_tuple_pattern": seq("{", ref("ex_atom"), ",", SP, ref("ex_name"), "}"),
    "ex_def": alt(
        seq(
            alt(kw("def"), kw("defp")), SP, ref("ex_function_name"), "(", opt(ref("ex_params")), ")",
            opt(seq(SP, kw("when"), SP, ref("ex_guard")), 0.25),
            SP, kw("do"), SP, ref("ex_expression"), SP, kw("end"),
        ),
        seq(
            alt(kw("def"), kw("defp")), SP, ref("ex_function_name"), "(", ref("ex_params"), "),",
            SP, "do:", SP, ref("ex_expression"),
        ),
    ),
    "ex_params": seq(ref("ex_param"), opt(seq(",", SP, ref("ex_param")))),
    "ex_param": alt(
        ref("ex_name"),
        ref("ex_name"),
        ref("ex_tuple_pattern"),
        seq("%{", ref("ex_name"), ":", SP, ref("ex_name"), "}"),
        seq("[head", SP, "|", SP, "tail]"),
        seq(ref("ex_name"), SP, "\\\\", SP, ref("ex_literal")),
    ),
    "ex_guard": alt(
        seq("is_", alt("integer", "binary", "list", "map", "atom"), "(", ref("ex_name"), ")"),
        seq(ref("ex_name"), SP, alt(">", "<", ">=", "=="), SP, ref("ex_int_or_name")),
    ),
    "ex_module": alt(
        seq(kw("defmodule"), SP, ref("ex_module_name"), SP, kw("do"), SP, ref("ex_module_body"), SP, kw("end")),
        seq(kw("defstruct"), SP, "[", ref("ex_atom"), ",", SP, ref("ex_atom"), "]"),
        seq(kw("alias"), SP, ref("ex_module_name")),
        seq(kw("use"), SP, alt("GenServer", "Ecto.Schema", "Phoenix.Controller", "ExUnit.Case")),
        seq(kw("import"), SP, alt("Ecto.Query", "Plug.Conn", "Enum", "String")),
    ),
    "ex_module_body": alt(
        seq("@moduledoc", SP, alt(kw("false"), flag("strings", ref("ex_string")))),
        seq(kw("use"), SP, "GenServer"),
        seq(kw("alias"), SP, ref("ex_module_name")),
        seq("@", ref("ex_name"), SP, ref("ex_literal")),
    ),
    "ex_module_name": seq(alt("MyApp", "Accounts", "Shop", "Blog", "Chat"), ".", ref("ex_class_name")),
    "ex_pipeline": seq(
        ref("ex_name"), SP, "|>", SP, ref("ex_stage"), SP, "|>", SP, ref("ex_stage"),
        opt(seq(SP, "|>", SP, ref("ex_stage")), 0.4),
    ),
    "ex_stage": alt(
        seq("Enum.", alt("map", "filter", "reject", "flat_map"), "(", ref("ex_fn"), ")"),
        seq("Enum.", alt("sum", "count", "sort", "uniq", "reverse"), "()"),
        seq("Enum.", alt("take", "drop", "chunk_every"), "(", ref("ex_int_or_name"), ")"),
        seq("String.", alt("trim", "downcase", "upcase", "split"), "()"),
        seq("Map.", alt("get", "fetch!", "delete"), "(", ref("ex_atom"), ")"),
        seq(ref("ex_function_name"), "()"),
    ),
    "ex_fn": alt(
        seq("&(&1", SP, alt("*", "+", "-"), SP, ref("ex_int_or_name"), ")"),
        seq(kw("fn"), SP, ref("ex_name"), SP, "->", SP, ref("ex_name"), ".", ref("ex_name"), SP, kw("end")),
        seq("&", ref("ex_function_name"), "/1"),
        seq("&String.", alt("upcase", "trim"), "/1"),
    ),
    "ex_case": seq(
        kw("case"), SP, ref("ex_call"), SP, kw("do"), SP, ref("ex_tuple_pattern"), SP, "->", SP,
        ref("ex_name"), SP, "{:error,", SP, "reason}", SP, "->", SP, "{:error,", SP, "reason}", SP, kw("end"),
    ),
    "ex_if": seq(
        alt(kw("if"), kw("unless")), SP, ref("ex_condition"), SP, kw("do"), SP, ref("ex_call"),
        opt(seq(SP, kw("else"), SP, ref("ex_call")), 0.4), SP, kw("end"),
    ),
    "ex_condition": alt(
        seq(ref("ex_name"), SP, alt("==", "!=", ">", "<", ">=", "<=", "==="), SP, ref("ex_literal")),
        seq(ref("ex_name"), SP, kw("in"), SP, "[", ref("ex_atom"), ",", SP, ref("ex_atom"), "]"),
        seq("is_nil(", ref("ex_name"), ")"),
    ),
    "ex_call": alt(
        seq(ref("ex_module_name"), ".", ref("ex_function_name"), "(", opt(ref("ex_args")), ")"),
        seq(ref("ex_function_name"), "(", opt(ref("ex_args")), ")"),
        seq("IO.puts(", ref("ex_simple"), ")"),
        seq("Logger.", alt("info", "debug", "error"), "(", ref("ex_simple"), ")"),
    ),
    "ex_args": seq(ref("ex_simple"), opt(seq(",", SP, ref("ex_simple")))),
    "ex_expression": alt(
        ref("ex_literal"),
        ref("ex_name"),
        ref("ex_call"),
        seq("{", ref("ex_atom"), ",", SP, ref("ex_simple"), "}"),
        seq("%{", ref("ex_name"), ":", SP, ref("ex_literal"), ",", SP, ref("ex_name"), ":", SP, ref("ex_literal"), "}"),
        seq("[", ref("ex_literal"), ",", SP, ref("ex_literal"), "]"),
        binary("ex_name", ["+", "-", "*", "<>", "++"], "ex_expression"),
        seq("%", ref("ex_class_name"), "{", ref("ex_name"), ":", SP, ref("ex_literal"), "}"),
        flag("strings", ref("ex_interpolation")),
    ),
    "ex_simple": alt(ref("ex_literal"), ref("ex_name")),
    "ex_literal": alt(
        flag("numbers", ref("ex_number")),
        flag("strings", ref("ex_string")),
        ref("ex_atom"),
        kw("nil"), kw("true"), kw("false"),
    ),
    "ex_atom": string(seq(":", ref("ex_name"))),
    "ex_number": number(alt(ref("integer_dec_literal"), ref("float_dec_literal"))),
    "ex_int_or_name": alt(flag("numbers", number(ref("integer_dec_literal"))), ref("ex_name")),
    "ex_string": string(seq('"', ref("generic_string_content"), '"')),
    "ex_interpolation": string(seq(
        '"', alt("Hello,", "Total:", "Got", "User"), SP, "#{", ref("ex_name"), "}", '"',
    )),
    "ex_spec": seq("@spec", SP, ref("ex_function_name"), "(", ref("ex_type"), ")", SP, "::", SP, ref("ex_type")),
    "ex_type": alt(
        "integer()", "String.t()", "boolean()", "map()", "list(integer())", "atom()",
        "{:ok, term()} | {:error, term()}", "keyword()",
    ),
    "ex_name": ref("generic_variable_name"),
    "ex_function_name": ref("snake_function_name"),
    "ex_class_name": ref("generic_class_name"),
    "ex_comment": comment(seq("# ", ref("comment_text"))),
})


# ---------------------------------------------------------------------------
# Lua

language("lua", "Lua", rules={
    "start": ref("lua_statement"),
    "lua_statement": alt(
        ref("lua_local"),
        ref("lua_local"),
        ref("lua_assign"),
        flag("defs", ref("lua_function")),
        ref("lua_if"),
        ref("lua_for"),
        ref("lua_while"),
        ref("lua_call"),
        ref("lua_return"),
        flag("comments", ref("lua_comment")),
    ),
    "lua_local": seq(
        kw("local"), SP, ref("lua_name"), opt(seq(",", SP, ref("lua_name")), 0.2),
        SP, "=", SP, ref("lua_expression"),
    ),
    "lua_assign": seq(
        alt(
            ref("lua_name"),
            seq(ref("lua_name"), ".", ref("lua_name")),
            seq(ref("lua_name"), "[", ref("lua_key"), "]"),
        ),
        SP, "=", SP, ref("lua_expression"),
    ),
    "lua_key": alt(ref("lua_name"), flag("numbers", ref("lua_int")), flag("strings", ref("lua_string"))),
    "lua_function": alt(
        seq(
            opt(seq(kw("local"), SP), 0.4), kw("function"), SP, ref("lua_function_name"),
            "(", opt(ref("lua_params")), ")", SP, ref("lua_body"), SP, kw("end"),
        ),
        seq(
            kw("function"), SP, ref("lua_class_name"), alt(".", ":"), ref("lua_function_name"),
            "(", opt(ref("lua_params")), ")", SP, ref("lua_body"), SP, kw("end"),
        ),
    ),
    "lua_params": alt(seq(ref("lua_name"), opt(seq(",", SP, ref("lua_name")))), "..."),
    "lua_body": alt(ref("lua_return"), ref("lua_call"), ref("lua_local")),
    "lua_if": seq(
        kw("if"), SP, ref("lua_condition"), SP, kw("then"), SP, ref("lua_call"),
        opt(seq(SP, kw("elseif"), SP, ref("lua_condition"), SP, kw("then"), SP, ref("lua_call")), 0.3),
        opt(seq(SP, kw("else"), SP, ref("lua_call")), 0.4), SP, kw("end"),
    ),
    "lua_for": alt(
        flag("numbers", seq(
            kw("for"), SP, ref("lua_name"), SP, "=", SP, ref("lua_int"), ",", SP,
            alt(ref("lua_int"), seq("#", ref("lua_name"))), SP, kw("do"), SP, ref("lua_call"), SP, kw("end"),
        )),
        seq(
            kw("for"), SP, alt("k, v", "i, v", "_, item", "key, value"), SP, kw("in"), SP,
            alt("pairs", "ipairs"), "(", ref("lua_name"), ")", SP, kw("do"), SP, ref("lua_call"), SP, kw("end"),
        ),
    ),
    "lua_while": seq(kw("while"), SP, ref("lua_condition"), SP, kw("do"), SP, ref("lua_assign"), SP, kw("end")),
    "lua_call": alt(
        seq(alt("print", "table.insert", "assert", "error", "string.format", "table.concat"), "(", ref("lua_args"), ")"),
        seq(ref("lua_name"), ":", ref("lua_function_name"), "(", opt(ref("lua_args")), ")"),
        seq(ref("lua_function_name"), "(", opt(ref("lua_args")), ")"),
    ),
    "lua_args": seq(ref("lua_simple"), opt(seq(",", SP, ref("lua_simple")))),
    "lua_return": seq(kw("return"), SP, ref("lua_expression")),
    "lua_condition": alt(
        seq(ref("lua_name"), SP, alt("==", "~=", ">", "<", ">=", "<="), SP, ref("lua_literal")),
        seq(kw("not"), SP, ref("lua_name")),
        seq(ref("lua_name"), SP, alt(kw("and"), kw("or")), SP, ref("lua_name")),
        seq("#", ref("lua_name"), SP, ">", SP, ref("lua_int_or_name")),
    ),
    "lua_expression": alt(
        ref("lua_literal"),
        ref("lua_name"),
        ref("lua_call"),
        ref("lua_table"),
        binary("lua_name", ["+", "-", "*", "/", "%", "..", "//"], "lua_expression"),
        seq(ref("lua_name"), SP, kw("or"), SP, ref("lua_literal")),
        seq("#", ref("lua_name")),
        seq(kw("function"), "(", ref("lua_name"), ")", SP, kw("return"), SP, ref("lua_name"), SP, kw("end")),
    ),
    "lua_simple": alt(ref("lua_literal"), ref("lua_name"), seq(ref("lua_name"), ".", ref("lua_name"))),
    "lua_table": alt(
        "{}",
        seq("{", SP, ref("lua_field"), opt(seq(",", SP, ref("lua_field"))), SP, "}"),
    ),
    "lua_field": alt(seq(ref("lua_name"), SP, "=", SP, ref("lua_literal")), ref("lua_literal")),
    "lua_literal": alt(
        flag("numbers", ref("lua_number")),
        flag("strings", ref("lua_string")),
        kw("nil"), kw("true"), kw("false"),
    ),
    "lua_number": number(alt(ref("integer_dec_literal"), ref("float_dec_literal"), ref("integer_hex_literal_2"))),
    "lua_int": number(ref("integer_dec_literal")),
    "lua_int_or_name": alt(flag("numbers", ref("lua_int")), ref("lua_name")),
    "lua_string": string(alt(
        seq('"', ref("generic_string_content"), '"'),
        seq("'", ref("generic_string_content"), "'"),
        seq("[[", ref("generic_string_content"), "]]"),
    )),
    "lua_name": ref("generic_variable_name"),
    "lua_function_name": ref("snake_function_name"),
    "lua_class_name": ref("generic_class_name"),
    "lua_comment": comment(seq("-- ", ref("comment_text"))),
})


# ---------------------------------------------------------------------------


def write(name, rules):
    text = json.dumps(rules, ensure_ascii=False)
    (OUT / f"{name}.json").write_text(text + "\n", encoding="utf-8")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    write("_names", NAMES)
    syntaxes = []
    for id, name, rules in LANGUAGES:
        write(f"lang_{id}", rules)
        grammar = [f"lang_{id}", "_names"] + SHARED
        syntaxes.append({"id": id, "name": name, "grammar": grammar, "start": "start"})
    text = json.dumps(syntaxes, indent=2, ensure_ascii=False)
    (OUT / "syntaxes.json").write_text(text + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
