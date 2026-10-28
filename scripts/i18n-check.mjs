// Checks the app's texts against the Turkish dictionary: every text given to t() or tn() must be in it, and no
// text a player reads may be left outside t(). `node scripts/i18n-check.mjs` exits non-zero on a missing text;
// `--loose` also lists literals that look like words a player reads, for a look by hand.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import ts from "typescript";

const loose = process.argv.includes("--loose");
const dictionary = new Set();
{
  const source = ts.createSourceFile("tr.ts", readFileSync("src/locales/tr.ts", "utf8"), ts.ScriptTarget.Latest, true);
  const visit = (node) => {
    if (ts.isPropertyAssignment(node)) {
      const key = node.name;
      if (ts.isStringLiteral(key) || ts.isIdentifier(key) || ts.isNoSubstitutionTemplateLiteral(key)) dictionary.add(key.text);
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
}

const files = [];
const walk = (dir) => {
  for (const f of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, f.name);
    if (f.isDirectory()) {
      if (f.name !== "locales") walk(path);
    } else if (/\.tsx?$/.test(f.name) && !f.name.endsWith(".d.ts")) files.push(path);
  }
};
walk("src");

const missing = [];
const suspects = [];
/** Calls whose string arguments are not read by players, or are looked up already. */
const QUIET_CALLS = new Set(["startsLike", "engineText", "invoke", "call", "getItem", "setItem", "removeItem", "querySelector", "addEventListener", "removeEventListener", "dispatchEvent", "CustomEvent", "split", "join", "replace", "includes", "startsWith", "endsWith", "toLocaleString", "toLocaleDateString", "toLocaleTimeString", "padStart", "matchAll", "match", "search", "test", "log", "error", "warn", "readSetting", "writeSetting", "useSetting", "fmt", "classList"]);
const QUIET_ATTRIBUTES = new Set(["className", "key", "type", "value", "href", "src", "d", "viewBox", "fill", "stroke", "strokeWidth", "strokeLinecap", "strokeLinejoin", "role", "id", "htmlFor", "name", "lang", "rel", "target", "loading", "style", "transform", "points", "x", "y", "width", "height", "textAnchor", "dominantBaseline", "data-theme", "mode"]);
const looksRead = (text) => /[A-Za-z]{2,}/.test(text) && (/\s/.test(text.trim()) || /^[A-Z][a-z]/.test(text.trim())) && !/^[a-z_.-]+$/.test(text) && !/^(var\(|#|M\d|https?:)/.test(text.trim());

for (const file of files) {
  const text = readFileSync(file, "utf8");
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, file.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const where = (node) => `${file}:${source.getLineAndCharacterOfPosition(node.getStart()).line + 1}`;
  const visit = (node) => {
    // A table whose texts are looked up where they are shown says so in its comment.
    if (ts.isVariableStatement(node) && (ts.getLeadingCommentRanges(text, node.getFullStart()) ?? []).some((c) => text.slice(c.pos, c.end).includes("translated where shown"))) return;
    if (ts.isCallExpression(node)) {
      const name = node.expression.getText(source).split(".").pop();
      if (name === "t" || name === "tn") {
        const literals = name === "t" ? [node.arguments[0]] : [node.arguments[1], node.arguments[2]];
        for (const a of literals) {
          for (const lit of a ? literalsOf(a) : []) if (!dictionary.has(lit.text)) missing.push(`${where(lit)}  ${lit.text}`);
        }
        node.arguments.slice(name === "t" ? 1 : 3).forEach(visit);
        return;
      }
      if (QUIET_CALLS.has(name)) return;
    }
    if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) return;
    if (ts.isJsxAttribute(node) && QUIET_ATTRIBUTES.has(node.name.getText(source))) return;
    if (ts.isBinaryExpression(node) && [ts.SyntaxKind.EqualsEqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsEqualsToken].includes(node.operatorToken.kind)) return;
    if (ts.isCaseClause(node)) return node.statements.forEach(visit);
    if (ts.isPropertyAssignment(node) && ["key", "kind", "tone", "className", "class"].includes(node.name.getText(source))) return;
    if (ts.isJsxText(node) && /[A-Za-z]{2,}/.test(node.text)) suspects.push(`${where(node)}  ${node.text.trim()}`);
    if ((ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) && looksRead(node.text)) suspects.push(`${where(node)}  ${node.text}`);
    if (ts.isTemplateExpression(node)) {
      const parts = [node.head.text, ...node.templateSpans.map((s) => s.literal.text)].join(" ");
      if (/[A-Za-z]{3,}/.test(parts.replace(/\b(px|ms|s|em|rem|deg|vh|vw)\b/g, ""))) suspects.push(`${where(node)}  \`${node.getText(source).slice(1, 120)}`);
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
}

/** The string literals a t() argument can be: a literal, or either side of a condition. */
function literalsOf(node) {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return [node];
  if (ts.isConditionalExpression(node)) return [...literalsOf(node.whenTrue), ...literalsOf(node.whenFalse)];
  if (ts.isParenthesizedExpression(node)) return literalsOf(node.expression);
  return [];
}

if (missing.length) {
  console.log(`Missing from src/locales/tr.ts (${missing.length}):`);
  for (const m of missing) console.log("  " + m);
}
if (loose && suspects.length) {
  console.log(`\nTexts outside t() (${suspects.length}):`);
  for (const s of suspects) console.log("  " + s);
}
if (!missing.length) console.log(`All ${dictionary.size} Turkish texts present for every t() call.`);
process.exit(missing.length ? 1 : 0);
