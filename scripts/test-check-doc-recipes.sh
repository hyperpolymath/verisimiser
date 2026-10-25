#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
# Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
# Regression checks for the documented-recipe inventory. Requires just.
set -eu

CHECKER="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/check-doc-recipes.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/docs" "$tmp/recipes"

cat > "$tmp/Justfile" <<'EOF'
import "recipes/imported.just"

@quiet:
    exit 99

with-arg arg:
    exit 99

alias shortcut := quiet
EOF
cat > "$tmp/recipes/imported.just" <<'EOF'
imported:
    exit 99
EOF
cat > "$tmp/unimported.just" <<'EOF'
unimported:
    exit 99
EOF
cat > "$tmp/docs/RSR_OUTLINE.adoc" <<'EOF'
----
just template-only
----
EOF
cat > "$tmp/README.adoc" <<'EOF'
Prose references such as just prose-only are ignored.
----
just quiet
just with-arg value
just shortcut
just imported
----
....
just quiet
....
EOF

sh "$CHECKER" "$tmp"

expect_failure() {
    expected_status="$1"
    expected_message="$2"
    status=0
    sh "$CHECKER" "$tmp" > "$tmp/result" 2>&1 || status=$?
    if [ "$status" -ne "$expected_status" ] ||
        ! grep -Fq "$expected_message" "$tmp/result"; then
        cat "$tmp/result" >&2
        echo "expected exit $expected_status containing: $expected_message" >&2
        exit 1
    fi
}

# An unimported file cannot make a documented recipe valid.
cat >> "$tmp/README.adoc" <<'EOF'
----
just unimported
----
EOF
expect_failure 1 'undefined recipe: just unimported'

# Undefined references in literal blocks are also checked.
cat > "$tmp/README.adoc" <<'EOF'
....
just missing
....
EOF
expect_failure 1 'undefined recipe: just missing'

# Lowercase justfiles are supported by Just too.
mv "$tmp/Justfile" "$tmp/justfile"
cat > "$tmp/README.adoc" <<'EOF'
----
just quiet
----
EOF
sh "$CHECKER" "$tmp"

# Listing errors must propagate rather than validating against a partial list.
printf '\ninvalid syntax\n' >> "$tmp/justfile"
expect_failure 1 'error:'

echo 'test-check-doc-recipes: OK'
