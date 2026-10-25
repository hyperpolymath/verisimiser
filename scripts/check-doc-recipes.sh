#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
# Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
#
# check-doc-recipes.sh — fail if documentation tells readers to run a
# `just` recipe that the Justfile does not define.
#
# Why: the quickstarts inherited from the RSR template referenced recipes
# that never existed here (`just setup`, `just e2e`, `just check-error-codes`,
# `just panic-scan`), so readers hit "Justfile does not contain recipe".
# ADR-0002 removed aspirational recipes; this check keeps the docs honest in
# the other direction.
#
# Scope: only `just <name>` occurrences inside AsciiDoc code blocks
# (`----` and `....` fences) are checked. Prose such as "just the eight
# concerns" is ignored because it is not inside a fence.
#
# Skipped on purpose (reported, not failed): template artifacts that describe
# the upstream RSR template repository rather than this project, and which are
# sync-managed from hyperpolymath/standards. Their recipes belong to the
# template, not to this Justfile.
#
# Usage: scripts/check-doc-recipes.sh [repo-root]   (default: this repo)

set -eu

ROOT="${1:-$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)}"
cd "$ROOT"

[ -f Justfile ] || [ -f justfile ] || {
    echo "check-doc-recipes: no Justfile in $ROOT" >&2
    exit 2
}

# Documents maintained upstream as template content, not as verisimiser docs.
SKIP="docs/RSR_OUTLINE.adoc"

# Print recipe-name candidates from the supplied file paths, one per line.
# With no paths, read standard input. Recognises `name:`, `name arg:`,
# `name *args:` and `name +args:`; this is not a full Justfile parser.
# Skips indented lines, comments, leading `@` lines and declarations using
# the `alias`/`import`/`export`/`set`/`mod` keywords.
# Returns awk's exit status, including failure to read an input file.
recipe_names() {
    awk '
        /^[ \t]/ { next }                       # indented body lines
        /^#/     { next }                        # comments
        /^@/     { next }                        # attributes/quiet lines
        /^(alias|import|export|set|mod)[ \t!?]/ { next }
        /^[a-zA-Z][a-zA-Z0-9_-]*([ \t][^:]*)?:/ {
            line = $0
            sub(/[ \t].*/, "", line)             # keep the leading token
            sub(/:.*/, "", line)
            if (line != "") print line
        }
    ' "$@"
}

recipes="$( {
    [ -f Justfile ] && recipe_names Justfile
    [ -f justfile ] && recipe_names justfile
    for f in *.just; do
        [ -f "$f" ] && recipe_names "$f"
    done
} | sort -u )"

[ -n "$recipes" ] || { echo "check-doc-recipes: no recipes found" >&2; exit 2; }

docs="$(ls ./*.adoc 2>/dev/null; find docs -name '*.adoc' 2>/dev/null)"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

for doc in $docs; do
    case " $(echo "$doc" | sed 's|^\./||') " in
        *" $SKIP "*)
            printf 'check-doc-recipes: skipping template artifact %s\n' "$doc"
            continue
            ;;
    esac
    awk -v DOC="$doc" '
        /^----[[:space:]]*$/ || /^\.\.\.\.[[:space:]]*$/ { infence = !infence; next }
        infence {
            rest = $0
            while (match(rest, /just[ \t]+[a-zA-Z][a-zA-Z0-9_-]*/)) {
                tok = substr(rest, RSTART, RLENGTH)
                sub(/^just[ \t]+/, "", tok)
                print DOC "\t" tok
                rest = substr(rest, RSTART + RLENGTH)
            }
        }
    ' "$doc" >> "$tmp"
done

bad=0
while IFS='	' read -r doc name; do
    [ -n "$name" ] || continue
    if ! printf '%s\n' "$recipes" | grep -qx "$name"; then
        printf 'undefined recipe: just %s   (%s)\n' "$name" "$doc"
        bad=1
    fi
done < "$tmp"

if [ "$bad" -ne 0 ]; then
    echo ""
    echo "check-doc-recipes: FAIL - documentation references recipes that do not exist." >&2
    echo "Either add the recipe, or fix the docs. Run 'just --list' for the real set." >&2
    exit 1
fi

echo "check-doc-recipes: OK - every documented 'just' recipe is defined."
