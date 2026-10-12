"""Repository naming check for cham3-authored Rust sources (AGENTS.md convention).

rustc's non_camel_case_types and non_upper_case_globals lints are allowed in the
workspace because they contradict the repository convention. This checker enforces
the repository rule instead, over every tracked or given .rs file under core/:

* functions, modules, local bindings and enum variants: snake_case;
* struct, enum, union, trait and module-level type alias names: snake_case ending
  in _t (associated types inside impl blocks are external trait boundaries);
* generic type parameters: snake_case ending in _t;
* const, static and macro_rules! names: PascalCase;
* no lowerCamelCase identifier anywhere in cham3 code;
* file and directory names under core/: snake_case (Cargo.toml is Cargo's
  required external spelling).

Usage: python3 -I check_rust_naming.py [FILE.rs ...]
With no arguments, checks every git-tracked .rs file and path under core/.
Prints one "VIOLATION <path>:<line>: <rule>" line per finding; exit 1 if any.
"""

import os
import re
import subprocess
import sys

SnakeCase = re.compile(r"^_?[a-z][a-z0-9_]*$")
SnakeTypeName = re.compile(r"^[a-z][a-z0-9_]*_t$")
PascalCase = re.compile(r"^[A-Z][A-Za-z0-9]*$")
LowerCamel = re.compile(r"^[a-z][a-z0-9]*[A-Z][A-Za-z0-9]*$")
# Identifiers not preceded by a digit or word character (excludes 0xA5, 1u8 suffixes).
IdentifierPattern = re.compile(r"(?<![0-9A-Za-z_])[A-Za-z_][A-Za-z0-9_]*")
ExternalFileNames = frozenset({"Cargo.toml", "Cargo.lock"})


def strip_comments_and_literals(text):
    """Replaces comments and string/char literal contents with spaces, keeping lines."""
    output = []
    index = 0
    length = len(text)
    while index < length:
        char = text[index]
        if text.startswith("//", index):
            end = text.find("\n", index)
            end = length if end < 0 else end
            output.append(" " * (end - index))
            index = end
            continue
        if text.startswith("/*", index):
            depth = 0
            start = index
            while index < length:
                if text.startswith("/*", index):
                    depth += 1
                    index += 2
                elif text.startswith("*/", index):
                    depth -= 1
                    index += 2
                    if depth == 0:
                        break
                else:
                    index += 1
            output.append(re.sub(r"[^\n]", " ", text[start:index]))
            continue
        raw = re.match(r'b?r(#*)"', text[index:])
        if raw and (index == 0 or not (text[index - 1].isalnum() or text[index - 1] == "_")):
            hashes = raw.group(1)
            terminator = '"' + hashes
            start = index
            end = text.find(terminator, index + raw.end())
            end = length if end < 0 else end + len(terminator)
            output.append(re.sub(r"[^\n]", " ", text[start:end]))
            index = end
            continue
        if char == '"' or (char == "b" and text.startswith('b"', index)):
            start = index
            index += 2 if char == "b" else 1
            while index < length and text[index] != '"':
                index += 2 if text[index] == "\\" else 1
            index += 1
            output.append(re.sub(r"[^\n]", " ", text[start:index]))
            continue
        if char == "'":
            literal = re.match(r"'(\\.|\\u\{[0-9a-fA-F]+\}|\\x[0-9a-fA-F]{2}|[^'\\])'", text[index:])
            if literal:
                output.append(" " * literal.end())
                index += literal.end()
                continue
        output.append(char)
        index += 1
    return "".join(output)


def line_of(text, offset):
    return text.count("\n", 0, offset) + 1


def block_kinds(code):
    """Maps each '{' offset to the kind of block it opens (impl/trait/enum/fn/mod/other)."""
    kinds = {}
    statement_start = 0
    for match in re.finditer(r"[{};]", code):
        if match.group() == "{":
            header = code[statement_start:match.start()]
            words = IdentifierPattern.findall(header)
            kind = "other"
            for word in ("impl", "trait", "enum", "union", "struct", "fn", "mod"):
                if word in words:
                    kind = word
                    break
            kinds[match.start()] = kind
        statement_start = match.end()
    return kinds


def enclosing_kind(code, offset, kinds):
    depth_stack = []
    for match in re.finditer(r"[{}]", code[:offset]):
        if match.group() == "{":
            depth_stack.append(kinds.get(match.start(), "other"))
        elif depth_stack:
            depth_stack.pop()
    return depth_stack[-1] if depth_stack else "module"


def check_source(path, text):
    violations = []
    code = strip_comments_and_literals(text)
    kinds = block_kinds(code)

    def report(offset, rule):
        violations.append("%s:%d: %s" % (path, line_of(code, offset), rule))

    for match in re.finditer(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)", code):
        if not SnakeCase.match(match.group(1)):
            report(match.start(1), "function %r is not snake_case" % match.group(1))
    for match in re.finditer(r"\bmod\s+([A-Za-z_][A-Za-z0-9_]*)", code):
        if not SnakeCase.match(match.group(1)):
            report(match.start(1), "module %r is not snake_case" % match.group(1))
    for match in re.finditer(r"\b(struct|enum|union|trait)\s+([A-Za-z_][A-Za-z0-9_]*)", code):
        if not SnakeTypeName.match(match.group(2)):
            report(match.start(2), "%s %r is not snake_case_t" % (match.group(1), match.group(2)))
    for match in re.finditer(r"\btype\s+([A-Za-z_][A-Za-z0-9_]*)", code):
        if enclosing_kind(code, match.start(), kinds) in ("impl", "trait"):
            continue
        if not SnakeTypeName.match(match.group(1)):
            report(match.start(1), "type alias %r is not snake_case_t" % match.group(1))
    for match in re.finditer(r"\b(const|static)\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*:", code):
        name = match.group(2)
        if name != "_" and not PascalCase.match(name):
            report(match.start(2), "%s %r is not PascalCase" % (match.group(1), name))
    for match in re.finditer(r"\bmacro_rules!\s*([A-Za-z_][A-Za-z0-9_]*)", code):
        if not PascalCase.match(match.group(1)):
            report(match.start(1), "macro %r is not PascalCase" % match.group(1))
    for match in re.finditer(r"\blet\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\b", code):
        name = match.group(1)
        if name not in ("_", "Some", "Ok", "Err", "None") and not SnakeCase.match(name):
            report(match.start(1), "binding %r is not snake_case" % name)
    # Enum variants: identifiers that start a variant inside an enum body.
    for match in re.finditer(r"[{,]\s*(?:#\[[^\]]*\]\s*)*([A-Za-z_][A-Za-z0-9_]*)\s*(?=[,({}=]|$)", code):
        if enclosing_kind(code, match.start(1), kinds) != "enum":
            continue
        if not SnakeCase.match(match.group(1)):
            report(match.start(1), "enum variant %r is not snake_case" % match.group(1))
    # Generic type parameters on items: <name_t, 'a, const Name: usize>.
    for match in re.finditer(r"\b(?:fn\s+\w+|struct\s+\w+|enum\s+\w+|trait\s+\w+|impl)\s*<([^<>]*(?:<[^<>]*>[^<>]*)*)>", code):
        for parameter in match.group(1).split(","):
            parameter = parameter.strip()
            if not parameter or parameter.startswith("'") or parameter.startswith("const "):
                continue
            name = re.split(r"[\s:=]", parameter, maxsplit=1)[0]
            if not IdentifierPattern.fullmatch(name):
                continue
            if not SnakeTypeName.match(name):
                report(match.start(1), "generic parameter %r is not snake_case_t" % name)
    for match in IdentifierPattern.finditer(code):
        if LowerCamel.match(match.group()):
            report(match.start(), "camelCase identifier %r" % match.group())
    return violations


def check_path_names(path):
    violations = []
    if not path.startswith("core/"):
        return violations
    for part in path.split("/"):
        if part in ExternalFileNames or part.startswith("."):
            continue
        stem = part.split(".", 1)[0]
        if not SnakeCase.match(stem):
            violations.append("%s: path component %r is not snake_case" % (path, part))
            break
    return violations


def tracked_core_paths(root):
    listing = subprocess.run(
        ["git", "-C", root, "ls-files", "-z", "--", "core"],
        stdout=subprocess.PIPE,
        check=True,
    ).stdout.decode("utf-8")
    return [path for path in listing.split("\0") if path]


def main(arguments):
    root = os.environ.get(
        "CHAM3_REPO_ROOT", os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    )
    if arguments:
        paths = arguments
        source_paths = [path for path in paths if path.endswith(".rs")]
        path_checks = []
    else:
        paths = tracked_core_paths(root)
        source_paths = [path for path in paths if path.endswith(".rs")]
        path_checks = paths
        if not source_paths:
            print("VIOLATION no tracked Rust sources under core/ (checker would pass vacuously)")
            return 1
    violations = []
    for path in path_checks:
        violations.extend(check_path_names(path))
    for path in source_paths:
        absolute = path if os.path.isabs(path) else os.path.join(root, path)
        with open(absolute, "r", encoding="utf-8") as handle:
            violations.extend(check_source(path, handle.read()))
    for violation in violations:
        print("VIOLATION " + violation)
    print("checked %d Rust files, %d paths: %s" % (len(source_paths), len(path_checks), "FAIL" if violations else "PASS"))
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
