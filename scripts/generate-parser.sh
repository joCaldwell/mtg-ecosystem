#!/bin/bash
# scripts/generate-parser.sh
# Regenerates ANTLR TypeScript parser from .g4 grammar files

set -e

# Resolve absolute paths to prevent ANTLR directory mirroring
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GRAMMAR_DIR="$ROOT_DIR/packages/oracle-parser/grammar"
OUTPUT_DIR="$ROOT_DIR/packages/oracle-parser/generated"

echo "Generating parser from grammar files..."
mkdir -p "$OUTPUT_DIR"

echo "1/2: Compiling lexer..."
npx antlr4ts-cli -o "$OUTPUT_DIR" "$GRAMMAR_DIR/MTGLexer.g4"

echo "2/2: Compiling parser (with imports)..."
npx antlr4ts-cli -lib "$OUTPUT_DIR" -o "$OUTPUT_DIR" -visitor "$GRAMMAR_DIR/MTGParser.g4"

echo "Done! Generated parser files in packages/oracle-parser/generated/"
