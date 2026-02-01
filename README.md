# textpp

textpp is a small text preprocessor for Markdown and other plain-text files. It supports a minimal set of directives and variable substitutions while leaving unknown `#` lines untouched.

## Use case

- Conditionally include or exclude sections of a Markdown document.
- Include other files relative to the current file.
- Substitute variables inside any file content.

## Supported syntax

Directives are recognized only when `#` is the first character on the line. All other `#...` lines are left as-is.

- `#include "relative/path.txt"`
  - Path is resolved relative to the current file.
  - `##VAR##` is replaced in the include path with `-DVAR=VALUE`.
  - Missing includes are ignored.
- `#ifdef VAR`
  - True when `VAR` is defined and not empty (`-DVAR=VALUE` or `-DVAR`).
  - `-DVAR=` or `-DVAR=""` makes `VAR` undefined.
- `#if (EXPR)`
  - Operators: `||`, `&&`, `!`, `==`, `!=`, parentheses.
  - Identifiers resolve to their defined value (or empty if undefined).
  - Truthiness: false when empty, `0`, `F`, `False`, or `NO` (case-insensitive). Otherwise true.
- `#else`
- `#endif`

Any mismatched `#if` / `#ifdef` / `#else` / `#endif` is a hard error. Invalid logical expressions are a hard error.

### Variable substitution

- `$$VAR$$` in any content is replaced with the defined value of `VAR`.
- If `VAR` is undefined, it is replaced with an empty string.

## CLI

```
textpp [-DKEY[=VALUE]] <input-file>
```

- `-DKEY` sets `KEY` to `TRUE`.
- `-DKEY=VALUE` sets `KEY` to `VALUE`.
- `-DKEY=` or `-DKEY=""` makes `KEY` undefined.

## Example

Input:

```
#ifdef NAME
Hello $$NAME$$
#else
Hello
#endif
```

Run:

```
textpp -DNAME=Alice input.md
```

Output:

```
Hello Alice
```
