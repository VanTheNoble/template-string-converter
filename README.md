# Template String Converter

A Zed extension that automatically converts string quotes to backticks when a template expression (`${...}`) is typed inside a JavaScript or TypeScript string.

## Example

```typescript
// Type this:
const greeting = "hello ${name}"

// Automatically becomes:
const greeting = `hello ${name}`
```

## Installation

### 1. Install the LSP binary

```bash
cargo install --path lsp-server
```

Make sure `~/.cargo/bin` is in your `PATH`.

### 2. Install the extension in Zed

**From the marketplace:** Search for "Template String Converter" in Zed's extension page.

**As a dev extension:**
1. Clone this repo.
2. Build and install the LSP binary (the compiled artifacts are not checked in, so you must build it yourself):
   ```bash
   cargo install --path lsp-server
   ```
   This places `template-string-converter-lsp` in `~/.cargo/bin`; make sure that directory is in your `PATH`. To iterate locally without installing, you can also build it with `cargo build --release` (the binary lands in `lsp-server/target/release/`) and put that directory on your `PATH`.
3. In Zed: Cmd+Shift+P → "Install Dev Extension" → select the cloned directory.

> **Note:** After rebuilding the LSP binary, restart the language server in Zed (Cmd+Shift+P → "editor: restart language server") so it picks up the new build. On Windows the binary may be locked while Zed is running it.

## How it works

The extension runs a lightweight LSP server that watches for `${` typed inside `"` or `'` strings. When detected, it sends an edit that:

1. Replaces both surrounding quotes with backticks, turning the string into a template literal.
2. Inserts the closing `}` itself when it isn't already there.

Everything runs through a single `didChange` scan of the changed region, so the conversion fires regardless of the order in which `$` and `{` are typed (e.g. typing `$` in front of an existing `{`).

The server inserts the `}` itself rather than relying on Zed's auto-pairing, because Zed only auto-closes `{` when the following character is in `autoclose_before` (whitespace, quotes, brackets) — never before a word character. The `}` is only inserted when it is not already present, so there is never a duplicate when Zed did auto-close it.

## Supported languages

- JavaScript
- TypeScript
- JSX
- TSX

## License

Apache 2.0
