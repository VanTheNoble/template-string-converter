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
1. Clone this repo
2. In Zed: Cmd+Shift+P → "Install Dev Extension" → select the cloned directory

## How it works

The extension runs a lightweight LSP server that watches for `${...}` patterns inside `"` or `'` strings. When detected, it sends an edit to replace both quotes with backticks, converting the string to a template literal.

The server uses two mechanisms:
- **`didChange` scanning** — detects `${...}` in the changed region of the document and proactively converts. Handles Zed's auto-pairing where `{` and `}` are inserted together.
- **`onTypeFormatting` fallback** — triggers on `{` for the case where the cursor is right before a closing quote (e.g., `"${|"`).

## Supported languages

- JavaScript
- TypeScript
- JSX
- TSX

## License

Apache 2.0
