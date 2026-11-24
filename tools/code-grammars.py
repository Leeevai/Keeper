#!/usr/bin/env python3
"""Writes the code grammars that Keeper adds to keybr.com's.

The output goes to assets/code/keeper, which the keybr.com extraction tool
leaves alone: one lang_<id>.json of rules per language, _camel.json with
camelCase names for the languages that use them, and syntaxes.json, the
list that Keeper loads after keybr's.

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


# camelCase names, for the languages that use them.
CAMEL = {
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
}

LANGUAGES = []


def language(id, name, rules, camel=False):
    """Registers a grammar; its `start` rule makes one statement."""
    for rule, body in rules.items():
        # An alternative that refers to this rule would still be picked
        # with the flag off, and print nothing: flag where it is used.
        assert "flag" not in body, f"{id}: move the flag of {rule} to where it is used"
    LANGUAGES.append((id, name, rules, camel))


# ---------------------------------------------------------------------------
# Kotlin

language("kotlin", "Kotlin", camel=True, rules={
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


def write(name, rules):
    text = json.dumps(rules, ensure_ascii=False)
    (OUT / f"{name}.json").write_text(text + "\n", encoding="utf-8")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    write("_camel", CAMEL)
    syntaxes = []
    for id, name, rules, camel in LANGUAGES:
        write(f"lang_{id}", rules)
        grammar = [f"lang_{id}"] + (["_camel"] if camel else []) + SHARED
        syntaxes.append({"id": id, "name": name, "grammar": grammar, "start": "start"})
    text = json.dumps(syntaxes, indent=2, ensure_ascii=False)
    (OUT / "syntaxes.json").write_text(text + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
