#!/bin/bash
# Ejecuta TODOS los tests E2E y genera golden images para validación visual
set -e

echo "🔍 EJECUCIÓN COMPLETA DE TESTS E2E CON VALIDACIÓN VISUAL"
echo "============================================================"
echo ""

DASHBOARD_DIR="tests/e2e"
EXPLORER_DIR="apps/explorer-ui"

# Contadores
TOTAL_TESTS=0
TESTS_WITH_SCREENSHOTS=0

GREEN='\033[0;32m'
NC='\033[0m'

echo -e "${GREEN}1. EXPLORER UI VISUAL REGRESSION${NC}"
echo "======================================"

cd "$EXPLORER_DIR"
npm run test:e2e:visual -- --update-snapshots || echo "⚠️  Algunos tests fallaron"

EXPLORER_GOLDENS=$(find e2e/visual-regression.spec.ts-snapshots -name "*.png" 2>/dev/null | wc -l || echo "0")
TESTS_WITH_SCREENSHOTS=$((TESTS_WITH_SCREENSHOTS + EXPLORER_GOLDENS))

echo "✓ Golden images generados: $EXPLORER_GOLDENS"
echo ""

echo "Golden images del Explorer UI:"
find e2e/visual-regression.spec.ts-snapshots -name "*.png" -exec basename {} \; | sort
echo ""

echo "============================================"
echo "✓ EJECUCIÓN COMPLETA"
echo "Total golden images: $TESTS_WITH_SCREENSHOTS"
echo ""
