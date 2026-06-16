#!/usr/bin/env bun

// Usage: bun scripts/convertFourslash.mts [inputFileList]
//
// Adapted from vendor/typescript-go/internal/fourslash/_scripts/convertFourslash.mts.
// Keep parser changes close to the TypeScript-Go source; only the paths and final
// Rust emission should diverge.

import * as fs from "fs";
import * as path from "path";
import * as ts from "typescript";
import * as url from "url";

const repoRoot = path.resolve(import.meta.dirname, "..");
const vendorRoot = path.join(repoRoot, "vendor", "typescript-go");
const typescriptRoot = path.join(vendorRoot, "_submodules", "TypeScript");
const stradaFourslashPath = path.join(typescriptRoot, "tests", "cases", "fourslash");

let inputFileSet: Set<string>;

const manualTestsPath = path.join(vendorRoot, "internal", "fourslash", "_scripts", "manualTests.txt");

let outputDir = path.join(repoRoot, "crates", "ts-fourslash", "src", "tests", "generated");

const unparsedFiles: { file: string; error: string; }[] = [];
let unparsedReportPath = path.join(outputDir, "unparsedTests.txt");
const generatedFiles: string[] = [];
const generatedFileBaseNameCounts = new Map<string, number>();
let updateExistingOutput = false;

// Code fix IDs that have been implemented in the Go port.
// Tests for code fixes not in this set will be skipped during conversion.
const allowedCodeFixIds = new Set([
    "fixMissingImport",
    "fixMissingTypeAnnotationOnExports",
    "fixClassIncorrectlyImplementsInterface",
]);

// File name prefixes for code fix tests that are allowed even without a fixId.
// These correspond to tests using verify.codeFix() or verify.codeFixAvailable()
// that don't include a fixId field.
const allowedCodeFixDescriptionPrefixes = [
    "Import ",
    "Add import from ",
    "Update import from ",
    "Implement interface '",
    "Change 'import' to 'import type'",
    "Add annotation of type",
    "Add return type",
    "Add satisfies and an inline type assertion",
    "Annotate types of properties expando function",
    "Extract default export to variable",
    "Extract base class to variable",
    "Extract binding expressions to variable",
    "Extract to variable and replace with",
    "Mark array literal as const",
];

// These fourslash APIs exercise TypeScript services that the current Rust harness
// does not expose or cannot validate through LSP-style requests yet. Skipping them
// keeps unparsedTests.txt focused on converter gaps instead of known unsupported
// feature families. Remove entries from this list when the corresponding harness
// behavior is implemented, so the source tests become eligible for conversion.
const impossibleFourslashCallPrefixes = [
    "verify.refactorAvailable",
    "verify.refactorAvailableForTriggerReason",
    "verify.refactorKindAvailable",
    "verify.refactorsAvailable",
    "verify.not.refactorAvailable",
    "verify.not.refactorAvailableForTriggerReason",
    "verify.moveToFile",
    "verify.moveToNewFile",
    "verify.noMoveToNewFile",
    "verify.pasteEdits",
    "verify.preparePasteEdits",
    "verify.baselineCurrentFileBreakpointLocations",
    "verify.baselineGetEmitOutput",
    "verify.getEmitOutput",
    "verify.baselineMapCode",
    "verify.docCommentTemplateAt",
    "verify.noDocCommentTemplateAt",
    "verify.todoCommentsInCurrentFile",
    "verify.toggleLineComment",
    "verify.toggleMultilineComment",
    "verify.uncommentSelection",
    "verify.getRegionSemanticDiagnostics",
    "verify.ProjectInfo",
    "verify.eval",
    "cancellation.setCancelled",
    "test.setTypesRegistry",
];

function getManualTests(): Set<string> {
    if (!fs.existsSync(manualTestsPath)) {
        return new Set();
    }
    const manualTestsList = fs.readFileSync(manualTestsPath, "utf-8").split("\n").map(line => line.trim()).filter(line => line.length > 0);
    return new Set(manualTestsList);
}

export async function main() {
    const args = process.argv.slice(2);
    let inputFilesPath: string | undefined;
    let outputDirWasExplicit = false;
    for (let i = 0; i < args.length; i++) {
        const arg = args[i];
        if (arg === "--output") {
            const outputArg = args[++i];
            if (!outputArg) {
                throw new Error("--output requires a path");
            }
            outputDir = path.resolve(outputArg);
            unparsedReportPath = path.join(outputDir, "unparsedTests.txt");
            outputDirWasExplicit = true;
        }
        else {
            inputFilesPath = arg;
        }
    }
    if (inputFilesPath) {
        const inputFiles = fs.readFileSync(inputFilesPath, "utf-8")
            .split("\n").map(line => line.trim())
            .filter(line => line.length > 0)
            .map(line => path.basename(line));
        inputFileSet = new Set(inputFiles);
    }

    if (!fs.existsSync(stradaFourslashPath) || fs.readdirSync(stradaFourslashPath).length === 0) {
        throw new Error(`Missing TypeScript fourslash source directory: ${stradaFourslashPath}.`);
    }

    updateExistingOutput = inputFileSet !== undefined && !outputDirWasExplicit;
    if (!updateExistingOutput) {
        fs.rmSync(outputDir, { recursive: true, force: true });
    }
    fs.mkdirSync(outputDir, { recursive: true });
    generatedFiles.length = 0;
    generatedFileBaseNameCounts.clear();

    parseTypeScriptFiles(getManualTests(), stradaFourslashPath);
    unparsedFiles.sort((a, b) => a.file.localeCompare(b.file, "en-US"));
    if (updateExistingOutput) {
        if (unparsedFiles.length > 0) {
            const errors = unparsedFiles.map(({ file, error }) => `${file} parse error: ${JSON.stringify(error)}`).join("\n");
            throw new Error(`Failed to parse ${unparsedFiles.length} selected files:\n${errors}`);
        }
    }
    else {
        fs.writeFileSync(path.join(outputDir, "mod.rs"), moduleLines(generatedFiles).join("\n") + "\n", "utf-8");
        fs.writeFileSync(unparsedReportPath, unparsedFiles.map(({ file, error }) => `${file} parse error: ${JSON.stringify(error)}`).join("\n"), "utf-8");
    }
    formatGeneratedRust();
    if (updateExistingOutput) {
        console.log(`Regenerated ${generatedFiles.length} selected files.`);
    }
    else {
        console.log(`Failed to parse ${unparsedFiles.length} files. See ${unparsedReportPath} for details.`);
    }
}

function hasTSExtension(file: string): boolean {
    return file.endsWith(".ts") ||
        file.endsWith(".tsx");
}

function parseTypeScriptFiles(manualTests: Set<string>, folder: string): void {
    const files = fs.readdirSync(folder);

    files.forEach(file => {
        const filePath = path.join(folder, file);
        const stat = fs.statSync(filePath);
        if (inputFileSet && !inputFileSet.has(file)) {
            return;
        }

        if (stat.isDirectory()) {
            parseTypeScriptFiles(manualTests, filePath);
        }
        else if (hasTSExtension(file) && !manualTests.has(file.slice(0, -3)) && file !== "fourslash.ts") {
            const content = fs.readFileSync(filePath, "utf-8");
            const isServer = filePath.split(path.sep).includes("server");
            try {
                const test = parseFileContent(file, content);
                if (test === NO_TEST) return;
                const testContent = generateRustTest(test, isServer);
                const generatedFile = generatedRustFileName(test.name);
                fs.writeFileSync(path.join(outputDir, generatedFile), testContent, "utf-8");
                if (!generatedFiles.includes(generatedFile)) {
                    generatedFiles.push(generatedFile);
                }
            }
            catch (e) {
                const message = e instanceof Error ? e.message : String(e);
                if (e instanceof SkipTest) {
                    if (updateExistingOutput) {
                        removeGeneratedRustFileForSource(file);
                    }
                    console.error(`Skipping file ${file}: ${message}`);
                    return;
                }
                console.error(`Error parsing file ${file}: ${message}`);
                unparsedFiles.push({ file, error: message });
            }
        }
    });
}

const NO_TEST: unique symbol = Symbol("NO_TEST");
type NoTest = typeof NO_TEST;

class SkipTest extends Error {
    constructor(message: string) {
        super(message);
        this.name = "SkipTest";
    }
}

function parseFileContent(filename: string, content: string): GoTest | NoTest {
    console.error(`Parsing file: ${filename}`);
    const sourceFile = ts.createSourceFile("temp.ts", content, ts.ScriptTarget.Latest, true /*setParentNodes*/);
    const statements = sourceFile.statements;
    const goTest: GoTest = {
        name: testNameFromFileName(filename),
        content: getTestInput(content),
        commands: [],
    };
    for (const statement of statements) {
        const result = parseFourslashStatement(statement);
        goTest.commands.push(...result);
    }
    if (goTest.commands.length === 0) {
        console.error(`No commands parsed in file (skipping): ${filename}`);
        return NO_TEST;
    }
    validateCodeFixCommands(goTest.commands);
    return goTest;
}

function validateCodeFixCommands(commands: Cmd[]): void {
    const hasCodeFixCmd = commands.some(c => c.kind === "verifyCodeFix" || c.kind === "verifyCodeFixAvailable" || c.kind === "verifyCodeFixAll");
    if (!hasCodeFixCmd) {
        return;
    }
    // Every codeFixAll must use an allowed fixId.
    for (const cmd of commands) {
        if (cmd.kind === "verifyCodeFixAll" && !allowedCodeFixIds.has(cmd.fixId)) {
            throw new SkipTest(`Unsupported code fix ID: ${cmd.fixId}`);
        }
    }
    // If there are codeFix/codeFixAvailable commands but no codeFixAll with an allowed ID,
    // the test is only accepted if its descriptions match allowed patterns.
    const hasAllowedCodeFixAll = commands.some(c => c.kind === "verifyCodeFixAll" && allowedCodeFixIds.has(c.fixId));
    const hasCodeFixOrAvailable = commands.some(c => c.kind === "verifyCodeFix" || c.kind === "verifyCodeFixAvailable");
    if (hasCodeFixOrAvailable && !hasAllowedCodeFixAll) {
        const allAllowed = commands.every(c => {
            if (c.kind === "verifyCodeFix") {
                return allowedCodeFixDescriptionPrefixes.some(p => c.description.startsWith(p));
            }
            if (c.kind === "verifyCodeFixAvailable") {
                // Empty descriptions means "assert no fixes available", which is always allowed.
                return c.descriptions.length === 0 || c.descriptions.every(d => allowedCodeFixDescriptionPrefixes.some(p => d.startsWith(p)));
            }
            return true;
        });
        if (!allAllowed) {
            throw new SkipTest(`Code fix test has no allowed fixId and descriptions do not match any allowed prefix`);
        }
    }
}

function getTestInput(content: string): string {
    const lines = content.split("\n").map(line => line.endsWith("\r") ? line.slice(0, -1) : line);
    let testInput: string[] = [];
    for (const line of lines) {
        let newLine = "";
        if (line.startsWith("////")) {
            const parts = line.substring(4).split("`");
            for (let i = 0; i < parts.length; i++) {
                if (i > 0) {
                    newLine += `\` + "\`" + \``;
                }
                newLine += parts[i];
            }
            testInput.push(newLine);
        }
        else if (line.startsWith("// @") || line.startsWith("//@")) {
            testInput.push(line);
        }
        // !!! preserve non-input comments?
    }

    // chomp leading spaces
    if (
        !testInput.some(line =>
            line.length != 0 &&
            !line.startsWith(" ") &&
            !line.startsWith("// ") &&
            !line.startsWith("//@")
        )
    ) {
        testInput = testInput.map(line => {
            if (line.startsWith(" ")) return line.substring(1);
            return line;
        });
    }
    return `\`${testInput.join("\n")}\``;
}

function getBadStatementText(statement: ts.Statement): string {
    if (ts.isExpressionStatement(statement) && ts.isCallExpression(statement.expression)) {
        return statement.expression.expression.getText() + "(...)";
    }
    return statement.getText();
}

function attachTrailingComments(commands: Cmd[], node: ts.Node): Cmd[] {
    const comments = trailingComments(node);
    if (comments.length === 0 || commands.length === 0) {
        return commands;
    }
    return commands.map((cmd, index) =>
        index === commands.length - 1 ? withComments(cmd, comments) : cmd
    );
}

function addTrailingComments<T extends CmdData>(cmd: T, node: ts.Node): Cmd {
    return withComments(cmd, trailingComments(node));
}

function withComments<T extends CmdData | Cmd>(cmd: T, comments: string[]): T & { comments?: string[] } {
    if (comments.length === 0) {
        return cmd;
    }
    return {
        ...cmd,
        comments: [...("comments" in cmd && cmd.comments ? cmd.comments : []), ...comments],
    };
}

function trailingComments(node: ts.Node): string[] {
    const sourceFile = node.getSourceFile();
    const ranges = ts.getTrailingCommentRanges(sourceFile.text, node.getEnd(sourceFile)) ?? [];
    return ranges.map(range => sourceFile.text.slice(range.pos, range.end).trimEnd());
}

interface VerifyAssertion {
    name: string;
    negated: boolean;
}

function parseVerifyAssertion(access: ts.PropertyAccessExpression): VerifyAssertion | undefined {
    if (ts.isIdentifier(access.expression) && access.expression.text === "verify") {
        return {
            name: access.name.text,
            negated: false,
        };
    }

    if (
        ts.isPropertyAccessExpression(access.expression) &&
        ts.isIdentifier(access.expression.expression) &&
        access.expression.expression.text === "verify" &&
        access.expression.name.text === "not"
    ) {
        return {
            name: access.name.text,
            negated: true,
        };
    }

    return undefined;
}

function isVerifyCompletionsCall(expression: ts.Expression): expression is ts.CallExpression {
    if (!ts.isCallExpression(expression) || !ts.isPropertyAccessExpression(expression.expression)) {
        return false;
    }
    const assertion = parseVerifyAssertion(expression.expression);
    return !!assertion && !assertion.negated && assertion.name === "completions";
}

interface ParseEnv {
    markerNamesVar?: string;
    rangesVar?: string;
    rangeDataNameAlias?: string;
    stringVars?: Record<string, string>;
}

function parseFourslashStatement(statement: ts.Statement, env: ParseEnv = {}): Cmd[] {
    return attachTrailingComments(parseFourslashStatementWorker(statement, env), statement);
}

function parseFourslashStatementWorker(statement: ts.Statement, env: ParseEnv = {}): Cmd[] {
    if (ts.isVariableStatement(statement)) {
        // variable declarations (for ranges and markers), e.g. `const range = test.ranges()[0];`
        return [];
    }
    else if (ts.isFunctionDeclaration(statement)) {
        // Helper functions are expanded at their call sites when supported.
        return [];
    }
    else if (ts.isEmptyStatement(statement)) {
        // Stray semicolons, e.g. `;;`
        return [];
    }
    else if (ts.isForOfStatement(statement)) {
        return parseForOfStatement(statement, env);
    }
    else if (ts.isExpressionStatement(statement) && ts.isCallExpression(statement.expression)) {
        const callExpression = statement.expression;
        skipImpossibleCall(callExpression);
        const forEachCommands = parseForEachCall(callExpression);
        if (forEachCommands) {
            return forEachCommands;
        }
        if (ts.isIdentifier(callExpression.expression)) {
            return parseHelperCall(callExpression);
        }
        if (!ts.isPropertyAccessExpression(callExpression.expression)) {
            throw new Error(`Expected property access expression, got ${callExpression.expression.getText()}`);
        }
        const accessExpression = callExpression.expression;
        const verifyAssertion = parseVerifyAssertion(accessExpression);

        if (verifyAssertion?.negated) {
            switch (verifyAssertion.name) {
                case "quickInfoExists":
                    return parseQuickInfoArgs("notQuickInfoExists", callExpression.arguments);
                case "codeFixAvailable":
                    return parseCodeFixAvailableArgs("notCodeFixAvailable", callExpression.arguments);
                case "codeFixAllAvailable":
                    return parseCodeFixAllAvailableArgs(callExpression.arguments);
                case "errorExistsAfterMarker":
                    return parseErrorExistsAfterMarker(callExpression.arguments).map(cmd => ({ ...cmd, kind: "verifyNoErrorExistsAfterMarker" }));
                case "errorExistsBetweenMarkers":
                    return parseErrorExistsBetweenMarkers(callExpression.arguments).map(cmd => ({ ...cmd, kind: "verifyNoErrorExistsBetweenMarkers" }));
                case "errorExistsBeforeMarker":
                    return parseErrorExistsBeforeMarker(callExpression.arguments).map(cmd => ({ ...cmd, kind: "verifyNoErrorExistsBeforeMarker" }));
            }
            throw new Error(`Unrecognized fourslash statement: ${getBadStatementText(statement)}`);
        }

        const expression = accessExpression.expression;
        if (isVerifyCompletionsCall(expression) && accessExpression.name.text === "andApplyCodeAction") {
            return parseVerifyCompletionsArgs(expression.arguments, callExpression.arguments);
        }

        // `verify.(...)`
        if (verifyAssertion) {
            switch (verifyAssertion.name) {
                case "completions":
                    // `verify.completions(...)`
                    return parseVerifyCompletionsArgs(callExpression.arguments);
                case "applyCodeActionFromCompletion":
                    // `verify.applyCodeActionFromCompletion(...)`
                    return parseVerifyApplyCodeActionFromCompletionArgs(callExpression.arguments);
                case "importFixAtPosition":
                    // `verify.importFixAtPosition(...)`
                    return parseImportFixAtPositionArgs(callExpression.arguments);
                case "importFixModuleSpecifiers":
                    // `verify.importFixModuleSpecifiers(...)`
                    return parseImportFixModuleSpecifiersArgs(callExpression.arguments);
                case "currentLineContentIs":
                case "currentFileContentIs":
                case "indentationIs":
                case "indentationAtPositionIs":
                case "textAtCaretIs":
                    return parseCurrentContentIsArgs(verifyAssertion.name, callExpression.arguments);
                case "quickInfoAt":
                case "quickInfoExists":
                case "quickInfoIs":
                case "quickInfos":
                    // `verify.quickInfo...(...)`
                    return parseQuickInfoArgs(verifyAssertion.name, callExpression.arguments, env);
                case "organizeImports":
                    // `verify.organizeImports(...)`
                    return parseOrganizeImportsArgs(callExpression.arguments);
                case "baselineFindAllReferences":
                    // `verify.baselineFindAllReferences(...)`
                    return parseBaselineFindAllReferencesArgs(callExpression.arguments);
                case "baselineDocumentHighlights":
                    return parseBaselineDocumentHighlightsArgs(callExpression.arguments);
                case "baselineCompletions":
                    return [{ kind: "verifyBaselineCompletions" }];
                case "baselineAutoImports":
                    return [{ kind: "verifyBaselineAutoImports" }];
                case "baselineQuickInfo":
                    return parseBaselineQuickInfo(callExpression.arguments);
                case "baselineSignatureHelp":
                    return [parseBaselineSignatureHelp(callExpression.arguments)];
                case "signatureHelp":
                    return parseSignatureHelp(callExpression.arguments);
                case "noSignatureHelp":
                    return parseNoSignatureHelp(callExpression.arguments);
                case "signatureHelpPresentForTriggerReason":
                    return parseSignatureHelpPresentForTriggerReason(callExpression.arguments);
                case "noSignatureHelpForTriggerReason":
                    return parseNoSignatureHelpForTriggerReason(callExpression.arguments);
                case "baselineSmartSelection":
                    return [parseBaselineSmartSelection(callExpression.arguments)];
                case "baselineCallHierarchy":
                    return [parseBaselineCallHierarchy(callExpression.arguments)];
                case "baselineGoToDefinition":
                case "baselineGetDefinitionAtPosition":
                case "baselineGoToType":
                case "baselineGoToImplementation":
                case "baselineGoToSourceDefinition":
                    // Both `baselineGoToDefinition` and `baselineGetDefinitionAtPosition` take the same
                    // arguments, but differ in that...
                    //  - `verify.baselineGoToDefinition(...)` called getDefinitionAndBoundSpan
                    //  - `verify.baselineGetDefinitionAtPosition(...)` called getDefinitionAtPosition
                    // LSP doesn't have two separate commands though.
                    return parseBaselineGoToDefinitionArgs(verifyAssertion.name, callExpression.arguments);
                case "baselineRename":
                case "baselineRenameAtRangesWithText":
                    // `verify.baselineRename...(...)`
                    return parseBaselineRenameArgs(verifyAssertion.name, callExpression.arguments);
                case "baselineInlayHints":
                    return parseBaselineInlayHints(callExpression.arguments);
                case "baselineLinkedEditing":
                    return [{ kind: "verifyBaselineLinkedEditing" }];
                case "linkedEditing":
                    return parseVerifyLinkedEditing(callExpression.arguments);
                case "renameInfoSucceeded":
                case "renameInfoFailed":
                    return parseRenameInfo(verifyAssertion.name, callExpression.arguments);
                case "getEditsForFileRename":
                    return parseGetEditsForFileRename(callExpression.arguments);
                case "getSemanticDiagnostics":
                case "getSuggestionDiagnostics":
                case "getSyntacticDiagnostics":
                    return parseVerifyDiagnostics(verifyAssertion.name, callExpression.arguments);
                case "baselineSyntacticDiagnostics":
                case "baselineSyntacticAndSemanticDiagnostics":
                    return [{ kind: "verifyBaselineDiagnostics" }];
                case "navigateTo":
                    return parseVerifyNavigateTo(callExpression.arguments, env);
                case "outliningSpansInCurrentFile":
                case "outliningHintSpansInCurrentFile":
                    return parseOutliningSpansArgs(callExpression.arguments);
                case "navigationTree":
                    return parseVerifyNavTree(callExpression.arguments);
                case "navigationBar":
                    return []; // Deprecated.
                case "numberOfErrorsInCurrentFile":
                    return parseNumberOfErrorsInCurrentFile(callExpression.arguments);
                case "noErrors":
                    return [{ kind: "verifyNoErrors" }];
                case "errorExistsAtRange":
                    return parseErrorExistsAtRange(callExpression.arguments);
                case "currentLineContentIs":
                    return parseCurrentLineContentIs(callExpression.arguments);
                case "currentFileContentIs":
                    return parseCurrentFileContentIs(callExpression.arguments);
                case "errorExistsBetweenMarkers":
                    return parseErrorExistsBetweenMarkers(callExpression.arguments);
                case "errorExistsAfterMarker":
                    return parseErrorExistsAfterMarker(callExpression.arguments);
                case "errorExistsBeforeMarker":
                    return parseErrorExistsBeforeMarker(callExpression.arguments);
                case "codeFix":
                    return parseCodeFixArgs(callExpression.arguments);
                case "codeFixAvailable":
                    return parseCodeFixAvailableArgs(verifyAssertion.name, callExpression.arguments);
                case "rangeAfterCodeFix":
                    return parseRangeAfterCodeFixArgs(callExpression.arguments);
                case "codeFixAll":
                    return parseCodeFixAllArgs(callExpression.arguments);
                case "semanticClassificationsAre":
                    return parseSemanticClassificationsAre(callExpression.arguments);
                case "syntacticClassificationsAre":
                    return [];
            }
        }

        if (!ts.isIdentifier(expression)) {
            throw new Error(`Unrecognized fourslash statement: ${getBadStatementText(statement)}`);
        }
        // `goTo....`
        if (expression.text === "goTo") {
            return parseGoToArgs(callExpression.arguments, accessExpression.name.text);
        }
        // `edit....`
        if (expression.text === "edit") {
            const result = parseEditStatement(accessExpression.name.text, callExpression.arguments);
            return [result];
        }
        if (expression.text === "format") {
            return parseFormatStatement(accessExpression.name.text, callExpression.arguments);
        }
        // !!! other fourslash commands
    }
    throw new Error(`Unrecognized fourslash statement: ${getBadStatementText(statement)}`);
}

function parseForEachCall(callExpression: ts.CallExpression): Cmd[] | undefined {
    if (!ts.isPropertyAccessExpression(callExpression.expression) || callExpression.expression.name.text !== "forEach") {
        return undefined;
    }
    if (callExpression.expression.expression.getText() !== "test.markers()") {
        return undefined;
    }
    const callback = callExpression.arguments[0];
    if (!callback || !ts.isArrowFunction(callback)) {
        throw new Error(`Unsupported test.markers().forEach callback: ${callback?.getText()}`);
    }
    const statements = ts.isBlock(callback.body)
        ? [...callback.body.statements]
        : [ts.factory.createExpressionStatement(callback.body)];
    if (
        statements.length === 1
        && ts.isExpressionStatement(statements[0]!)
        && ts.isCallExpression(statements[0]!.expression)
        && statements[0]!.expression.expression.getText() === "verify.indentationAtPositionIs"
        && (
            statements[0]!.expression.arguments.map(arg => arg.getText()).join(",") === "marker.fileName,marker.position,marker.data.indent"
            || statements[0]!.expression.arguments.map(arg => arg.getText()).join(",") === "marker.fileName,marker.position,marker.data.indentation"
        )
    ) {
        return [{ kind: "verifyIndentationAtMarkersFromData" }];
    }
    throw new Error(`Unsupported test.markers().forEach body: ${callback.body.getText()}`);
}

function skipImpossibleCall(callExpression: ts.CallExpression): void {
    const callee = callExpression.expression.getText();
    if (impossibleFourslashCallPrefixes.some(prefix => callee === prefix || callee.startsWith(`${prefix}.`))) {
        throw new SkipTest(`Unsupported fourslash feature: ${callee}`);
    }
}

function parseHelperCall(callExpression: ts.CallExpression): Cmd[] {
    const helperName = (callExpression.expression as ts.Identifier).text;
    switch (helperName) {
        case "verifyIndentationAfterNewLine": {
            const [markerArg, indentationArg] = callExpression.arguments;
            const marker = markerArg && getStringLiteralLike(markerArg);
            const indentation = indentationArg && getNumericLiteral(indentationArg);
            if (!marker || !indentation) {
                throw new Error(`Expected (string, number) arguments in ${helperName}, got ${callExpression.arguments.map(arg => arg.getText()).join(", ")}`);
            }
            return [
                { kind: "goTo", funcName: "marker", marker: marker.text },
                { kind: "edit", action: "insert", text: "\n" },
                { kind: "verifyContent", assertion: "indentationIs", text: indentation.text },
            ];
        }
        default:
            throw new Error(`Unrecognized fourslash helper call: ${helperName}(...)`);
    }
}

function parseForOfStatement(statement: ts.ForOfStatement, env: ParseEnv): Cmd[] {
    if (!ts.isVariableDeclarationList(statement.initializer) || statement.initializer.declarations.length !== 1) {
        throw new Error(`Unsupported for-of initializer: ${statement.initializer.getText()}`);
    }
    const declaration = statement.initializer.declarations[0]!;
    if (!ts.isIdentifier(declaration.name)) {
        throw new Error(`Unsupported for-of binding: ${declaration.name.getText()}`);
    }
    const varName = declaration.name.text;
    const expressionText = statement.expression.getText();
    if (expressionText === "test.markerNames()") {
        return parseLoopBody(statement, { ...env, markerNamesVar: varName });
    }
    else if (expressionText === "test.ranges()") {
        return parseLoopBody(statement, { ...env, rangesVar: varName });
    }
    else if (ts.isArrayLiteralExpression(statement.expression)) {
        const bodyStatements = getLoopBodyStatements(statement);
        return statement.expression.elements.flatMap(element => {
            const value = getStaticStringExpression(element, env);
            if (value === undefined) {
                throw new Error(`Unsupported for-of array element: ${element.getText()}`);
            }
            const elementEnv: ParseEnv = {
                ...env,
                stringVars: { ...env.stringVars, [varName]: value },
            };
            return bodyStatements.flatMap(bodyStatement => parseFourslashStatement(bodyStatement, elementEnv));
        });
    }

    throw new Error(`Unsupported for-of expression: ${expressionText}`);
}

function getLoopBodyStatements(statement: ts.ForOfStatement): ts.Statement[] {
    return ts.isBlock(statement.statement)
        ? [...statement.statement.statements]
        : [statement.statement];
}

function parseLoopBody(statement: ts.ForOfStatement, env: ParseEnv): Cmd[] {
    const commands: Cmd[] = [];
    let currentEnv = env;
    for (const bodyStatement of getLoopBodyStatements(statement)) {
        const updatedEnv = updateLoopEnvFromStatement(bodyStatement, currentEnv);
        if (updatedEnv) {
            currentEnv = updatedEnv;
            continue;
        }
        commands.push(...parseFourslashStatement(bodyStatement, currentEnv));
    }
    return commands;
}

function updateLoopEnvFromStatement(statement: ts.Statement, env: ParseEnv): ParseEnv | undefined {
    if (!env.rangesVar || !ts.isVariableStatement(statement)) return undefined;
    for (const declaration of statement.declarationList.declarations) {
        if (
            ts.isObjectBindingPattern(declaration.name)
            && declaration.initializer?.getText() === `${env.rangesVar}.marker.data`
        ) {
            for (const element of declaration.name.elements) {
                if (
                    ts.isBindingElement(element)
                    && ts.isIdentifier(element.name)
                    && element.name.text === "name"
                    && !element.propertyName
                ) {
                    return { ...env, rangeDataNameAlias: element.name.text };
                }
            }
        }
    }
    return undefined;
}

function parseEditStatement(funcName: string, args: readonly ts.Expression[]): EditCmd {
    switch (funcName) {
        case "applyRefactor":
            throw new SkipTest(`Unsupported fourslash feature: edit.${funcName}`);
        case "disableFormatting":
            return {
                kind: "edit",
                action: "disableFormatting",
            };
        case "insert":
        case "paste":
        case "insertLine": {
            let arg0;
            if (args.length !== 1 || !(arg0 = getStringLiteralLike(args[0]!))) {
                throw new Error(`Expected a single string literal argument in edit.${funcName}, got ${args.map(arg => arg.getText()).join(", ")}`);
            }
            return {
                kind: "edit",
                action: funcName,
                text: arg0.text,
            };
        }
        case "replaceLine": {
            let arg0, arg1;
            if (args.length !== 2 || !(arg0 = getNumericLiteral(args[0]!)) || !(arg1 = getStringLiteralLike(args[1]!))) {
                throw new Error(`Expected a single string literal argument in edit.insert, got ${args.map(arg => arg.getText()).join(", ")}`);
            }
            return {
                kind: "edit",
                action: "replaceLine",
                line: Number(arg0.text),
                text: arg1.text,
            };
        }
        case "backspace": {
            const arg = args[0]!;
            if (args[0]!) {
                let arg0;
                if (!(arg0 = getNumericLiteral(arg))) {
                    throw new Error(`Expected numeric literal argument in edit.backspace, got ${arg.getText()}`);
                }
                return {
                    kind: "edit",
                    action: "backspace",
                    count: Number(arg0.text),
                };
            }
            return {
                kind: "edit",
                action: "backspace",
                count: 1,
            };
        }
        case "deleteLine":
        case "deleteAtCaret": {
            const arg = args[0]!;
            if (arg) {
                let arg0;
                if (arg0 = getNumericLiteral(arg)) {
                    return {
                        kind: "edit",
                        action: funcName,
                        count: Number(arg0.text),
                    };
                }
                // Handle 'string'.length expressions
                const lengthValue = getStringLengthExpression(arg);
                if (lengthValue !== undefined) {
                    return {
                        kind: "edit",
                        action: funcName,
                        count: lengthValue,
                    };
                }
                throw new Error(`Expected numeric literal argument in edit.${funcName}, got ${arg.getText()}`);
            }
            return {
                kind: "edit",
                action: funcName,
                count: 1,
            };
        }
        default:
            throw new Error(`Unrecognized edit function: ${funcName}`);
    }
}

function parseFormatStatement(funcName: string, args: readonly ts.Expression[]): FormatCmd[] {
    switch (funcName) {
        case "document": {
            return [{
                kind: "format",
                action: "document",
            }];
        }
        case "setOption":
            var optName = getStringLiteralLike(args[0]!)!.text;
            if (optName == "newline") {
                optName = "NewLineCharacter";
            }
            var optValue = args[1]!.getText();
            if (
                (args[1]!.kind == ts.SyntaxKind.TrueKeyword || args[1]!.kind == ts.SyntaxKind.FalseKeyword)
            ) {
                optValue = stringToTristate(args[1]!.getText());
            }
            return [{
                kind: "format",
                action: "setOption",
                option: optName,
                value: optValue,
            }];
        case "setFormatOptions": {
            const obj = getObjectLiteralExpression(args[0]!);
            if (!obj) {
                return [];
            }
            const commands: FormatCmd[] = [];
            for (const prop of obj.properties) {
                if (ts.isSpreadAssignment(prop)) {
                    continue;
                }
                if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) {
                    throw new Error(`Unsupported format option property: ${prop.getText()}`);
                }
                commands.push({
                    kind: "format",
                    action: "setOption",
                    option: prop.name.text,
                    value: prop.initializer.kind === ts.SyntaxKind.TrueKeyword || prop.initializer.kind === ts.SyntaxKind.FalseKeyword
                        ? stringToTristate(prop.initializer.getText())
                        : prop.initializer.getText(),
                });
            }
            return commands;
        }
        case "selection": {
            const startMarker = getStringLiteralLike(args[0]!)?.text;
            const endMarker = getStringLiteralLike(args[1]!)?.text;
            if (startMarker === undefined || endMarker === undefined) {
                throw new Error(`format.selection: expected two string literal marker names`);
            }
            return [{
                kind: "format",
                action: "selection",
                startMarker,
                endMarker,
            }];
        }
        case "onType":
        case "copyFormatOptions":
        case "setFormatOptions":
        default:
            throw new Error(`Unrecognized format function: ${funcName}`);
    }
}

function parseCurrentContentIsArgs(funcName: string, args: readonly ts.Expression[]): VerifyContentCmd[] {
    switch (funcName) {
        case "currentFileContentIs":
            return [{
                kind: "verifyContent",
                assertion: "currentFileContentIs",
                text: getStringLiteralTextFromNode(args[0]!),
            }];
        case "currentLineContentIs":
            return [{
                kind: "verifyContent",
                assertion: "currentLineContentIs",
                text: getStringLiteralTextFromNode(args[0]!),
            }];
        case "indentationIs":
            return [{
                kind: "verifyContent",
                assertion: "indentationIs",
                text: getNumericLiteral(args[0]!)?.text ?? args[0]!.getText(),
            }];
        case "indentationAtPositionIs":
        case "textAtCaretIs":
        default:
            throw new Error(`Unrecognized verify content function: ${funcName}`);
    }
}

function getGoStringLiteral(text: string): string {
    return `${JSON.stringify(text)}`;
}

function getStringLiteralTextFromNode(node: ts.Node): string {
    const stringLiteralLike = getStringLiteralLike(node);
    if (stringLiteralLike) {
        return stringLiteralLike.text;
    }
    switch (node.kind) {
        case ts.SyntaxKind.BinaryExpression: {
            const binaryExpr = node as ts.BinaryExpression;
            const left = getStringLiteralTextFromNode(binaryExpr.left);
            const right = getStringLiteralTextFromNode(binaryExpr.right);
            const op = binaryExpr.operatorToken.getText();
            if (op !== "+") {
                throw new Error(`Unhandled binary operator ${op} in string literal expression: ${node.getText()}`);
            }
            return left + right;
        }
        default:
            throw new Error(`Unhandled case ${node.kind} in getStringLiteralTextFromNode: ${node.getText()}`);
    }
}

function parseGoToArgs(args: readonly ts.Expression[], funcName: string): GoToCmd[] {
    switch (funcName) {
        case "marker": {
            const arg = args[0]!;
            if (arg === undefined) {
                return [{
                    kind: "goTo",
                    funcName: "marker",
                    marker: "",
                }];
            }
            let strArg;
            if (!(strArg = getStringLiteralLike(arg))) {
                throw new Error(`Unrecognized argument in goTo.marker: ${arg.getText()}`);
            }
            return [{
                kind: "goTo",
                funcName: "marker",
                marker: strArg.text,
            }];
        }
        case "file": {
            if (args.length !== 1) {
                throw new Error(`Expected a single argument in goTo.file, got ${args.map(arg => arg.getText()).join(", ")}`);
            }
            let arg0;
            if (arg0 = getStringLiteralLike(args[0]!)) {
                const text = arg0.text.replace("tests/cases/fourslash/server/", "").replace("tests/cases/fourslash/", "");
                return [{
                    kind: "goTo",
                    funcName: "file",
                    file: text,
                }];
            }
            else if (arg0 = getNumericLiteral(args[0]!)) {
                return [{
                    kind: "goTo",
                    funcName: "fileNumber",
                    fileNumber: Number(arg0.text),
                }];
            }
            throw new Error(`Expected string or number literal argument in goTo.file, got ${args[0]!.getText()}`);
        }
        case "position": {
            let arg0;
            if (args.length !== 1 || !(arg0 = getNumericLiteral(args[0]!))) {
                throw new Error(`Expected a single numeric literal argument in goTo.position, got ${args.map(arg => arg.getText()).join(", ")}`);
            }
            return [{
                kind: "goTo",
                funcName: "position",
                position: Number(arg0.text),
            }];
        }
        case "eof":
            return [{
                kind: "goTo",
                funcName: "EOF",
            }];
        case "bof":
            return [{
                kind: "goTo",
                funcName: "BOF",
            }];
        case "select": {
            let arg0, arg1;
            if (args.length !== 2 || !(arg0 = getStringLiteralLike(args[0]!)) || !(arg1 = getStringLiteralLike(args[1]!))) {
                throw new Error(`Expected two string literal arguments in goTo.select, got ${args.map(arg => arg.getText()).join(", ")}`);
            }
            return [{
                kind: "goTo",
                funcName: "select",
                startMarker: arg0.text,
                endMarker: arg1.text,
            }];
        }
        default:
            throw new Error(`Unrecognized goTo function: ${funcName}`);
    }
}

function parseVerifyCompletionsArgs(args: readonly ts.Expression[], codeActionArgs?: readonly ts.Expression[]): VerifyCompletionsCmd[] {
    const cmds = [];
    const codeAction = codeActionArgs?.[0] && parseAndApplyCodeActionArg(codeActionArgs[0]!);
    for (const arg of args) {
        const result = parseVerifyCompletionArg(arg, codeAction);
        if (codeActionArgs?.length) {
            result.andApplyCodeActionArgs = parseAndApplyCodeActionArg(codeActionArgs[0]!);
        }
        cmds.push(result);
    }
    return cmds;
}

function parseVerifyApplyCodeActionFromCompletionArgs(args: readonly ts.Expression[]): VerifyApplyCodeActionFromCompletionCmd[] {
    const cmds: VerifyApplyCodeActionFromCompletionCmd[] = [];
    if (args.length !== 2) {
        throw new Error(`Expected two arguments in verify.applyCodeActionFromCompletion, got ${args.map(arg => arg.getText()).join(", ")}`);
    }
    if (!ts.isStringLiteralLike(args[0]!) && args[0]!.getText() !== "undefined") {
        throw new Error(`Expected string literal or "undefined" in verify.applyCodeActionFromCompletion, got ${args[0]!.getText()}`);
    }
    const markerName = getStringLiteralLike(args[0]!)?.text;
    const options = parseVerifyApplyCodeActionArgs(args[1]!);

    cmds.push({ kind: "verifyApplyCodeActionFromCompletion", marker: markerName, options });
    return cmds;
}

function parseVerifyApplyCodeActionArgs(arg: ts.Expression): ApplyCodeActionFromCompletionOptions {
    const obj = getObjectLiteralExpression(arg);
    if (!obj) {
        throw new Error(`Expected object literal for verify.applyCodeActionFromCompletion options, got ${arg.getText()}`);
    }
    let name: string | undefined;
    let source: string | undefined;
    let description: string | undefined;
    let autoImportFix = false;
    let newFileContent: string | undefined;
    let newRangeContent: string | undefined;
    for (const prop of obj.properties) {
        if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) {
            if (ts.isShorthandPropertyAssignment(prop) && prop.name.text === "preferences") {
                continue; // !!! parse once preferences are supported in fourslash
            }
            throw new Error(`Expected property assignment with identifier name in verify.applyCodeActionFromCompletion options, got ${prop.getText()}`);
        }
        const propName = prop.name.text;
        const init = prop.initializer;
        switch (propName) {
            case "name":
                if (!(name = getStringLiteralLike(init)?.text)) {
                    throw new Error(`Expected string literal for name in verify.applyCodeActionFromCompletion options, got ${init.getText()}`);
                }
                break;
            case "source":
                if (source = getCompletionSourceConstant(init)) {
                    break;
                }
                if (!(source = getStringLiteralLike(init)?.text)) {
                    throw new Error(`Expected string literal for source in verify.applyCodeActionFromCompletion options, got ${init.getText()}`);
                }
                break;
            case "data":
                const dataInit = getObjectLiteralExpression(init);
                if (!dataInit) {
                    throw new Error(`Expected object literal for data in verify.applyCodeActionFromCompletion options, got ${init.getText()}`);
                }
                for (const dataProp of dataInit.properties) {
                    if (!ts.isPropertyAssignment(dataProp) || !ts.isIdentifier(dataProp.name)) {
                        throw new Error(`Expected property assignment with identifier name in verify.applyCodeActionFromCompletion data, got ${dataProp.getText()}`);
                    }
                    const dataPropName = dataProp.name.text;
                    switch (dataPropName) {
                        case "moduleSpecifier":
                            const moduleSpecifierInit = getStringLiteralLike(dataProp.initializer);
                            if (!moduleSpecifierInit) {
                                throw new Error(`Expected string literal for moduleSpecifier in verify.applyCodeActionFromCompletion data, got ${dataProp.initializer.getText()}`);
                            }
                            break;
                    }
                }
                autoImportFix = true;
                break;
            case "description":
                if (!(description = getStringLiteralLike(init)?.text)) {
                    throw new Error(`Expected string literal for description in verify.applyCodeActionFromCompletion options, got ${init.getText()}`);
                }
                break;
            case "newFileContent":
                const newFileContentInit = getStringLiteralLike(init);
                if (!newFileContentInit) {
                    throw new Error(`Expected string literal for newFileContent in verify.applyCodeActionFromCompletion options, got ${init.getText()}`);
                }
                newFileContent = newFileContentInit.text;
                break;
            case "newRangeContent":
                const newRangeContentInit = getStringLiteralLike(init);
                if (!newRangeContentInit) {
                    throw new Error(`Expected string literal for newRangeContent in verify.applyCodeActionFromCompletion options, got ${init.getText()}`);
                }
                newRangeContent = newRangeContentInit.text;
                break;
            case "preferences":
                // Few if any tests use non-default preferences
                break;
            default:
                throw new Error(`Unrecognized property in verify.applyCodeActionFromCompletion options: ${prop.getText()}`);
        }
    }
    if (name === undefined) {
        throw new Error(`Expected name property in verify.applyCodeActionFromCompletion options`);
    }
    if (source === undefined && !autoImportFix) {
        throw new Error(`Expected source property in verify.applyCodeActionFromCompletion options`);
    }
    if (description === undefined) {
        throw new Error(`Expected description property in verify.applyCodeActionFromCompletion options`);
    }
    return {
        name,
        source: source ?? "",
        description,
        autoImportFix,
        newFileContent,
        newRangeContent,
    };
}

function getCompletionSourceConstant(expr: ts.Expression): string | undefined {
    const text = expr.getText();
    const prefix = "completion.CompletionSource.";
    if (!text.startsWith(prefix)) {
        return undefined;
    }
    switch (text.slice(prefix.length)) {
        case "ThisProperty":
            return "ThisProperty/";
        case "ClassMemberSnippet":
            return "ClassMemberSnippet/";
        case "TypeOnlyAlias":
            return "TypeOnlyAlias/";
        case "ObjectLiteralMethodSnippet":
            return "ObjectLiteralMethodSnippet/";
        case "SwitchCases":
            return "SwitchCases/";
        case "ObjectLiteralMemberWithComma":
            return "ObjectLiteralMemberWithComma/";
        default:
            throw new Error(`Unrecognized completion source constant: ${text}`);
    }
}

function parseImportFixAtPositionArgs(args: readonly ts.Expression[]): VerifyImportFixAtPositionCmd[] {
    if (args.length < 1 || args.length > 3) {
        throw new Error(`Expected 1-3 arguments in verify.importFixAtPosition, got ${args.map(arg => arg.getText()).join(", ")}`);
    }
    const arrayArg = getArrayLiteralExpression(args[0]!);
    if (!arrayArg) {
        throw new Error(`Expected array literal for first argument in verify.importFixAtPosition, got ${args[0]!.getText()}`);
    }
    const expectedTexts: string[] = [];
    for (const elem of arrayArg.elements) {
        const strElem = getStringLiteralLike(elem);
        if (!strElem) {
            throw new Error(`Expected string literal in verify.importFixAtPosition array, got ${elem.getText()}`);
        }
        expectedTexts.push(strElem.text);
    }

    // If the array is empty, we should still generate valid Go code
    if (expectedTexts.length === 0) {
        expectedTexts.push(""); // This will be handled specially in code generation
    }

    let preferences: string | undefined;
    if (args.length > 2 && ts.isObjectLiteralExpression(args[2]!)) {
        preferences = parseUserPreferences(args[2]!);
    }
    return [{
        kind: "verifyImportFixAtPosition",
        expectedTexts,
        preferences: preferences || "nil /*preferences*/",
    }];
}

function parseImportFixModuleSpecifiersArgs(args: readonly ts.Expression[]): [VerifyImportFixModuleSpecifiersCmd] {
    if (args.length < 2 || args.length > 3) {
        throw new Error(`Expected 2-3 arguments in verify.importFixModuleSpecifiers, got ${args.length}`);
    }

    const markerArg = getStringLiteralLike(args[0]!);
    if (!markerArg) {
        throw new Error(`Expected string literal for marker in verify.importFixModuleSpecifiers, got ${args[0]!.getText()}`);
    }
    const markerName = markerArg.text;

    const arrayArg = getArrayLiteralExpression(args[1]!);
    if (!arrayArg) {
        throw new Error(`Expected array literal for module specifiers in verify.importFixModuleSpecifiers, got ${args[1]!.getText()}`);
    }

    const moduleSpecifiers: string[] = [];
    for (const elem of arrayArg.elements) {
        const strElem = getStringLiteralLike(elem);
        if (!strElem) {
            throw new Error(`Expected string literal in module specifiers array, got ${elem.getText()}`);
        }
        moduleSpecifiers.push(strElem.text);
    }

    let preferences = "nil /*preferences*/";
    if (args.length > 2 && ts.isObjectLiteralExpression(args[2]!)) {
        const parsedPrefs = parseUserPreferences(args[2]!);
        preferences = parsedPrefs;
    }

    return [{
        kind: "verifyImportFixModuleSpecifiers",
        markerName,
        moduleSpecifiers,
        preferences,
    }];
}

const completionConstants = new Map([
    ["completion.globals", "CompletionGlobals"],
    ["completion.globalTypes", "CompletionGlobalTypes"],
    ["completion.classElementKeywords", "CompletionClassElementKeywords"],
    ["completion.classElementInJsKeywords", "CompletionClassElementInJSKeywords"],
    ["completion.constructorParameterKeywords", "CompletionConstructorParameterKeywords"],
    ["completion.functionMembersWithPrototype", "CompletionFunctionMembersWithPrototype"],
    ["completion.functionMembers", "CompletionFunctionMembers"],
    ["completion.typeKeywords", "CompletionTypeKeywords"],
    ["completion.undefinedVarEntry", "CompletionUndefinedVarItem"],
    ["completion.typeAssertionKeywords", "CompletionTypeAssertionKeywords"],
    ["completion.globalThisEntry", "CompletionGlobalThisItem"],
]);

const completionPlus = new Map([
    ["completion.globalsPlus", "CompletionGlobalsPlus"],
    ["completion.globalTypesPlus", "CompletionGlobalTypesPlus"],
    ["completion.functionMembersPlus", "CompletionFunctionMembersPlus"],
    ["completion.functionMembersWithPrototypePlus", "CompletionFunctionMembersWithPrototypePlus"],
    ["completion.globalsInJsPlus", "CompletionGlobalsInJSPlus"],
    ["completion.typeKeywordsPlus", "CompletionTypeKeywordsPlus"],
]);

function parseVerifyCompletionArg(arg: ts.Expression, codeActionArgs?: VerifyApplyCodeActionArgs): VerifyCompletionsCmd {
    let marker: CompletionMarkerInput | undefined;
    let goArgs: VerifyCompletionsArgs | undefined;
    const defaultGoArgs: VerifyCompletionsArgs = { preferences: "nil /*preferences*/" };
    const obj = getObjectLiteralExpression(arg);
    if (!obj) {
        throw new Error(`Expected object literal expression in verify.completions, got ${arg.getText()}`);
    }
    let isNewIdentifierLocation: true | undefined;
    for (const prop of obj.properties) {
        if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) {
            if (ts.isShorthandPropertyAssignment(prop) && prop.name.text === "preferences") {
                continue; // !!! parse once preferences are supported in fourslash
            }
            throw new Error(`Expected property assignment with identifier name, got ${prop.getText()}`);
        }
        const propName = prop.name.text;
        const init = prop.initializer;
        switch (propName) {
            case "marker": {
                marker = parseCompletionMarkerInput(init);
                break;
            }
            case "exact":
            case "includes":
            case "unsorted": {
                if (init.getText() === "undefined") {
                    return {
                        kind: "verifyCompletions",
                        marker: marker ?? { kind: "none" },
                        args: "nil",
                    };
                }
                let expected: string;
                const initText = init.getText();
                if (completionConstants.has(initText)) {
                    expected = completionConstants.get(initText)!;
                }
                else if (completionPlus.keys().some(funcName => initText.startsWith(funcName))) {
                    const tsFunc = completionPlus.keys().find(funcName => initText.startsWith(funcName));
                    const funcName = completionPlus.get(tsFunc!)!;
                    const maybeItems = (init as ts.CallExpression).arguments[0]!;
                    const maybeOpts = (init as ts.CallExpression).arguments[1];
                    let items;
                    if (!(items = getArrayLiteralExpression(maybeItems))) {
                        throw new Error(`Expected array literal expression for completion.globalsPlus items, got ${maybeItems.getText()}`);
                    }
                    expected = `${funcName}(\n[]fourslash.CompletionsExpectedItem{`;
                    for (const elem of items.elements) {
                        const result = parseExpectedCompletionItem(elem, codeActionArgs);
                        expected += "\n" + result + ",";
                    }
                    expected += "\n}";
                    if (maybeOpts) {
                        let opts;
                        if (!(opts = getObjectLiteralExpression(maybeOpts))) {
                            throw new Error(`Expected object literal expression for completion.globalsPlus options, got ${maybeOpts.getText()}`);
                        }
                        const noLib = opts.properties[0];
                        if (noLib && ts.isPropertyAssignment(noLib) && noLib.name.getText() === "noLib") {
                            if (noLib.initializer.kind === ts.SyntaxKind.TrueKeyword) {
                                expected += ", true";
                            }
                            else if (noLib.initializer.kind === ts.SyntaxKind.FalseKeyword) {
                                expected += ", false";
                            }
                            else {
                                throw new Error(`Expected boolean literal for noLib, got ${noLib.initializer.getText()}`);
                            }
                        }
                        else {
                            throw new Error(`Expected noLib property in completion.globalsPlus options, got ${maybeOpts.getText()}`);
                        }
                    }
                    else if (tsFunc === "completion.globalsPlus" || tsFunc === "completion.globalsInJsPlus") {
                        expected += ", false"; // Default for noLib
                    }
                    expected += ")";
                }
                else {
                    expected = "[]fourslash.CompletionsExpectedItem{";
                    let items;
                    if (items = getArrayLiteralExpression(init)) {
                        for (const elem of items.elements) {
                            const result = parseExpectedCompletionItem(elem);
                            expected += "\n" + result + ",";
                        }
                    }
                    else {
                        const result = parseExpectedCompletionItem(init);
                        expected += "\n" + result + ",";
                    }
                    expected += "\n}";
                }
                if (propName === "includes") {
                    (goArgs ??= defaultGoArgs).includes = expected;
                }
                else if (propName === "exact") {
                    (goArgs ??= defaultGoArgs).exact = expected;
                }
                else {
                    (goArgs ??= defaultGoArgs).unsorted = expected;
                }
                break;
            }
            case "excludes": {
                const excludes: string[] = [];
                let item;
                if (item = getStringLiteralLike(init)) {
                    excludes.push(item.text);
                }
                else if (item = getArrayLiteralExpression(init)) {
                    for (const elem of item.elements) {
                        if (!ts.isStringLiteral(elem)) {
                            throw new Error(`Expected string literal in excludes array, got ${elem.getText()}`);
                        }
                        excludes.push(elem.text);
                    }
                }
                (goArgs ??= defaultGoArgs).excludes = excludes;
                break;
            }
            case "isNewIdentifierLocation":
                if (init.kind === ts.SyntaxKind.TrueKeyword) {
                    isNewIdentifierLocation = true;
                }
                break;
            case "preferences": {
                if (!ts.isObjectLiteralExpression(init)) {
                    throw new Error(`Expected object literal for user preferences, got ${init.getText()}`);
                }
                const preferences = parseUserPreferences(init);
                (goArgs ??= defaultGoArgs).preferences = preferences;
                break;
            }
            case "triggerCharacter":
                break; // !!! parse once they're supported in fourslash
            case "defaultCommitCharacters":
            case "optionalReplacementSpan": // the only two tests that use this will require manual conversion
            case "isGlobalCompletion":
                break; // Ignored, unused
            default:
                throw new Error(`Unrecognized expected completion item: ${init.parent.getText()}`);
        }
    }
    return {
        kind: "verifyCompletions",
        marker: marker ?? { kind: "none" },
        args: goArgs,
        isNewIdentifierLocation: isNewIdentifierLocation,
    };
}

function parseCompletionMarkerInput(init: ts.Expression): CompletionMarkerInput {
    let markerInit;
    if (markerInit = getStringLiteralLike(init)) {
        return { kind: "name", name: markerInit.text };
    }
    if (markerInit = getArrayLiteralExpression(init)) {
        const names: string[] = [];
        for (const elem of markerInit.elements) {
            if (!ts.isStringLiteral(elem)) {
                throw new Error(`Expected string literal in marker array, got ${elem.getText()}`);
            }
            names.push(elem.text);
        }
        return { kind: "names", names };
    }
    if (markerInit = getObjectLiteralExpression(init)) {
        // !!! parse marker objects?
        throw new Error(`Unrecognized marker initializer: ${markerInit.getText()}`);
    }
    if (init.getText() === "test.markers()") {
        return { kind: "allMarkers" };
    }
    if (
        ts.isCallExpression(init)
        && init.expression.getText() === "test.marker"
        && ts.isStringLiteralLike(init.arguments[0]!)
    ) {
        return { kind: "marker", name: init.arguments[0]!.text };
    }
    throw new Error(`Unrecognized marker initializer: ${init.getText()}`);
}

function parseExpectedCompletionItem(expr: ts.Expression, codeActionArgs?: VerifyApplyCodeActionArgs): string {
    if (completionConstants.has(expr.getText())) {
        return completionConstants.get(expr.getText())!;
    }
    let strExpr;
    if (strExpr = getStringLiteralLike(expr)) {
        return getGoStringLiteral(strExpr.text);
    }
    if (strExpr = getObjectLiteralExpression(expr)) {
        let isOptional = false;
        const completionItemTags = new Set<string>();
        let sourceInit: ts.StringLiteralLike | undefined;
        let itemProps: string[] = [];
        let name: string | undefined;
        let insertText: string | undefined;
        let filterText: string | undefined;
        let replacementSpanIdx: string | undefined;
        for (const prop of strExpr.properties) {
            if (!(ts.isPropertyAssignment(prop) || ts.isShorthandPropertyAssignment(prop)) || !ts.isIdentifier(prop.name)) {
                throw new Error(`Expected property assignment with identifier name for completion item, got ${prop.getText()}`);
            }
            const propName = prop.name.text;
            const init = ts.isPropertyAssignment(prop) ? prop.initializer : prop.name;
            switch (propName) {
                case "name": {
                    let nameInit;
                    if (nameInit = getStringLiteralLike(init)) {
                        name = nameInit.text;
                    }
                    else {
                        throw new Error(`Expected string literal for completion item name, got ${init.getText()}`);
                    }
                    break;
                }
                case "sortText":
                    const sortText = parseSortText(init);
                    itemProps.push(`SortText: new(string(${sortText.expression})),`);
                    if (sortText.expression === "ls.SortTextOptionalMember") {
                        isOptional = true;
                    }
                    if (sortText.deprecated) {
                        completionItemTags.add("lsproto.CompletionItemTagDeprecated");
                    }
                    break;
                case "insertText": {
                    let insertTextInit;
                    if (insertTextInit = getStringLiteralLike(init)) {
                        insertText = insertTextInit.text;
                    }
                    else if (init.getText() === "undefined") {
                        // Ignore
                    }
                    else {
                        throw new Error(`Expected string literal for insertText, got ${init.getText()}`);
                    }
                    break;
                }
                case "filterText": {
                    let filterTextInit;
                    if (filterTextInit = getStringLiteralLike(init)) {
                        filterText = filterTextInit.text;
                    }
                    else {
                        throw new Error(`Expected string literal for filterText, got ${init.getText()}`);
                    }
                    break;
                }
                case "isRecommended":
                    if (init.kind === ts.SyntaxKind.TrueKeyword) {
                        itemProps.push(`Preselect: new(true),`);
                    }
                    break;
                case "kind":
                    const kind = parseKind(init);
                    itemProps.push(`Kind: new(${kind}),`);
                    break;
                case "kindModifiers":
                    const modifiers = parseKindModifiers(init);
                    ({ isOptional } = modifiers);
                    if (modifiers.isDeprecated) {
                        completionItemTags.add("lsproto.CompletionItemTagDeprecated");
                    }
                    break;
                case "text": {
                    let textInit;
                    if (textInit = getStringLiteralLike(init)) {
                        itemProps.push(`Detail: new(${getGoStringLiteral(textInit.text)}),`);
                    }
                    else {
                        throw new Error(`Expected string literal for text, got ${init.getText()}`);
                    }
                    break;
                }
                case "documentation": {
                    let docInit;
                    if (docInit = getStringLiteralLike(init)) {
                        itemProps.push(`Documentation: &lsproto.StringOrMarkupContent{
						MarkupContent: &lsproto.MarkupContent{
							Kind:  lsproto.MarkupKindMarkdown,
							Value: ${getGoStringLiteral(docInit.text)},
						},
					},`);
                    }
                    else {
                        throw new Error(`Expected string literal for documentation, got ${init.getText()}`);
                    }
                    break;
                }
                case "isFromUncheckedFile":
                    break; // Ignored
                case "hasAction":
                case "isPackageJsonImport":
                    break;
                case "source":
                case "sourceDisplay":
                    if (sourceInit !== undefined) {
                        break;
                    }
                    if (sourceInit = getStringLiteralLike(init)) {
                        if (propName === "source" && sourceInit.text.endsWith("/")) {
                            // source: "ClassMemberSnippet/"
                            itemProps.push(`Data: &lsproto.CompletionItemData{
                                Source: ${getGoStringLiteral(sourceInit.text)},
                            },`);
                            break;
                        }
                        itemProps.push(`Data: &lsproto.CompletionItemData{
                            AutoImport: &lsproto.AutoImportFix{
                                ModuleSpecifier: ${getGoStringLiteral(sourceInit.text)},
                            },
                        },`);
                    }
                    else if (init.getText().startsWith("completion.CompletionSource.")) {
                        const source = init.getText().slice("completion.CompletionSource.".length);
                        switch (source) {
                            // Ignore switch snippet sources
                            case "SwitchCases": {
                                continue;
                            }
                            case "ClassMemberSnippet":
                            case "ObjectLiteralMemberWithComma":
                            case "TypeOnlyAlias":
                            case "ThisProperty": {
                                continue;
                            }
                            default:
                                throw new Error(`Unrecognized source in expected completion item: ${init.getText()}`);
                        }
                    }
                    else {
                        throw new Error(`Expected string literal for source/sourceDisplay, got ${init.getText()}`);
                    }
                    break;
                case "commitCharacters":
                    // !!! support these later
                    break;
                case "replacementSpan": {
                    let span;
                    if (ts.isIdentifier(init)) {
                        span = getNodeOfKind(init, (n: ts.Node): n is ts.Node => !ts.isIdentifier(n));
                    }
                    else {
                        span = init;
                    }
                    if (span?.getText().startsWith("test.ranges()[")) {
                        replacementSpanIdx = span.getText().match(/\d+/)?.[0];
                    }
                    break;
                }
                case "isSnippet":
                    if (init.kind === ts.SyntaxKind.TrueKeyword) {
                        itemProps.push(`InsertTextFormat: new(lsproto.InsertTextFormatSnippet),`);
                    }
                    break;
                default:
                    throw new Error(`Unrecognized property in expected completion item: ${propName}`); // Unsupported property
            }
        }
        if (!name) {
            throw new Error(`Expected name property in expected completion item`);
        }
        if (codeActionArgs && codeActionArgs.name === name && codeActionArgs.source === sourceInit?.text) {
            itemProps.push(`LabelDetails: &lsproto.CompletionItemLabelDetails{
                Description: new(${getGoStringLiteral(codeActionArgs.source)}),
            },`);
        }
        if (replacementSpanIdx) {
            itemProps.push(`TextEdit: &lsproto.TextEditOrInsertReplaceEdit{
                TextEdit: &lsproto.TextEdit{
                    NewText: ${getGoStringLiteral(name)},
                    Range:   f.Ranges()[${replacementSpanIdx}].LSRange,
                },
            },`);
        }
        if (isOptional) {
            insertText ??= name;
            filterText ??= name;
            name += "?";
        }
        if (filterText) itemProps.unshift(`FilterText: new(${getGoStringLiteral(filterText)}),`);
        if (insertText) itemProps.unshift(`InsertText: new(${getGoStringLiteral(insertText)}),`);
        const tags = formatCompletionItemTags(completionItemTags);
        if (tags) itemProps.push(tags);
        itemProps.unshift(`Label: ${getGoStringLiteral(name!)},`);
        return `&lsproto.CompletionItem{\n${itemProps.join("\n")}}`;
    }
    throw new Error(`Expected string literal or object literal for expected completion item, got ${expr.getText()}`); // Unsupported expression type
}

function parseAndApplyCodeActionArg(arg: ts.Expression): VerifyApplyCodeActionArgs {
    const obj = getObjectLiteralExpression(arg);
    if (!obj) {
        throw new Error(`Expected object literal for code action argument, got ${arg.getText()}`);
    }
    const nameProperty = obj.properties.find(prop =>
        ts.isPropertyAssignment(prop) &&
        ts.isIdentifier(prop.name) &&
        prop.name.text === "name" &&
        ts.isStringLiteralLike(prop.initializer)
    ) as ts.PropertyAssignment;
    if (!nameProperty) {
        throw new Error(`Expected name property in code action argument, got ${obj.getText()}`);
    }
    const sourceProperty = obj.properties.find(prop =>
        ts.isPropertyAssignment(prop) &&
        ts.isIdentifier(prop.name) &&
        prop.name.text === "source" &&
        ts.isStringLiteralLike(prop.initializer)
    ) as ts.PropertyAssignment;
    if (!sourceProperty) {
        throw new Error(`Expected source property in code action argument, got ${obj.getText()}`);
    }
    const descriptionProperty = obj.properties.find(prop =>
        ts.isPropertyAssignment(prop) &&
        ts.isIdentifier(prop.name) &&
        prop.name.text === "description" &&
        ts.isStringLiteralLike(prop.initializer)
    ) as ts.PropertyAssignment;
    if (!descriptionProperty) {
        throw new Error(`Expected description property in code action argument, got ${obj.getText()}`);
    }
    const newFileContentProperty = obj.properties.find(prop =>
        ts.isPropertyAssignment(prop) &&
        ts.isIdentifier(prop.name) &&
        prop.name.text === "newFileContent" &&
        ts.isStringLiteralLike(prop.initializer)
    ) as ts.PropertyAssignment;
    if (!newFileContentProperty) {
        throw new Error(`Expected newFileContent property in code action argument, got ${obj.getText()}`);
    }
    return {
        name: (nameProperty.initializer as ts.StringLiteralLike).text,
        source: (sourceProperty.initializer as ts.StringLiteralLike).text,
        description: (descriptionProperty.initializer as ts.StringLiteralLike).text,
        newFileContent: (newFileContentProperty.initializer as ts.StringLiteralLike).text,
    };
}

function parseBaselineFindAllReferencesArgs(args: readonly ts.Expression[]): [VerifyBaselineFindAllReferencesCmd] {
    const markers: BaselineMarkerArg[] = [];
    for (const arg of args) {
        let strArg;
        if (strArg = getStringLiteralLike(arg)) {
            markers.push({ kind: "name", name: strArg.text });
        }
        else if (arg.getText() === "...test.markerNames()") {
            markers.push({ kind: "allMarkerNames" });
        }
        else if (arg.getText() === "...test.ranges()") {
            return [{
                kind: "verifyBaselineFindAllReferences",
                markers: [],
                ranges: true,
            }];
        }
        else {
            throw new Error(`Unrecognized argument in verify.baselineFindAllReferences: ${arg.getText()}`);
        }
    }

    return [{
        kind: "verifyBaselineFindAllReferences",
        markers,
    }];
}

function parseBaselineDocumentHighlightsArgs(args: readonly ts.Expression[]): [VerifyBaselineDocumentHighlightsCmd] {
    const newArgs: string[] = [];
    let preferences: string | undefined;
    let filesToSearch: string[] | undefined;
    for (const arg of args) {
        let strArg;
        if (strArg = getArrayLiteralExpression(arg)) {
            for (const elem of strArg.elements) {
                const newArg = parseBaselineMarkerOrRangeArg(elem);
                newArgs.push(newArg);
            }
        }
        else if (ts.isCallExpression(arg) && arg.getText().includes("test.ranges()")) {
            newArgs.push("ToAny(f.Ranges())...");
        }
        else if (ts.isObjectLiteralExpression(arg)) {
            for (const prop of arg.properties) {
                if (ts.isPropertyAssignment(prop) && ts.isIdentifier(prop.name) && prop.name.text === "filesToSearch" && ts.isArrayLiteralExpression(prop.initializer)) {
                    filesToSearch = [];
                    for (const e of prop.initializer.elements) {
                        if (ts.isStringLiteral(e)) {
                            filesToSearch.push(JSON.stringify(e.text));
                        }
                        else if (ts.isPropertyAccessExpression(e) && e.name.text === "fileName") {
                            // e.g. test.ranges()[0].fileName -> f.Ranges()[0].FileName()
                            const obj = e.expression;
                            if (ts.isElementAccessExpression(obj) && ts.isCallExpression(obj.expression) && obj.expression.getText().includes("ranges")) {
                                const index = obj.argumentExpression?.getText();
                                if (index !== undefined) {
                                    filesToSearch.push(`f.Ranges()[${index}].FileName()`);
                                    continue;
                                }
                            }
                            // e.g. range.fileName where `const range = test.ranges()[0]`
                            if (ts.isIdentifier(obj)) {
                                const resolved = parseRangeVariable(obj);
                                if (resolved) {
                                    filesToSearch.push(`${resolved}.FileName()`);
                                    continue;
                                }
                            }
                            // Fallback: skip filesToSearch entirely
                            filesToSearch = undefined;
                            break;
                        }
                        else {
                            // Unsupported expression; skip filesToSearch
                            filesToSearch = undefined;
                            break;
                        }
                    }
                }
            }
        }
        else {
            newArgs.push(parseBaselineMarkerOrRangeArg(arg));
        }
    }

    if (newArgs.length === 0) {
        newArgs.push("ToAny(f.Ranges())...");
    }

    return [{
        kind: "verifyBaselineDocumentHighlights",
        args: newArgs,
        preferences: preferences ? preferences : "nil /*preferences*/",
        filesToSearch,
    }];
}

function parseBaselineGoToDefinitionArgs(
    funcName: "baselineGoToDefinition" | "baselineGoToType" | "baselineGetDefinitionAtPosition" | "baselineGoToImplementation" | "baselineGoToSourceDefinition",
    args: readonly ts.Expression[],
): [VerifyBaselineGoToDefinitionCmd] {
    let boundSpan: true | undefined;
    if (funcName === "baselineGoToDefinition") {
        boundSpan = true;
    }
    let kind: "verifyBaselineGoToDefinition" | "verifyBaselineGoToType" | "verifyBaselineGoToImplementation" | "verifyBaselineGoToSourceDefinition";
    switch (funcName) {
        case "baselineGoToDefinition":
        case "baselineGetDefinitionAtPosition":
            kind = "verifyBaselineGoToDefinition";
            break;
        case "baselineGoToType":
            kind = "verifyBaselineGoToType";
            break;
        case "baselineGoToImplementation":
            kind = "verifyBaselineGoToImplementation";
            break;
        case "baselineGoToSourceDefinition":
            kind = "verifyBaselineGoToSourceDefinition";
            break;
    }
    const markers: BaselineMarkerArg[] = [];
    for (const arg of args) {
        let strArg;
        if (strArg = getStringLiteralLike(arg)) {
            markers.push({ kind: "name", name: strArg.text });
        }
        else if (arg.getText() === "...test.markerNames()") {
            markers.push({ kind: "allMarkerNames" });
        }
        else if (arg.getText() === "...test.ranges()") {
            return [{
                kind,
                markers: [],
                ranges: true,
                boundSpan,
            }];
        }
        else {
            throw new Error(`Unrecognized argument in verify.${funcName}: ${arg.getText()}`);
        }
    }

    return [{
        kind,
        markers,
        boundSpan,
    }];
}

function parseRenameInfo(funcName: "renameInfoSucceeded" | "renameInfoFailed", args: readonly ts.Expression[]): [VerifyRenameInfoCmd] {
    let preferences = "nil /*preferences*/";
    let prefArg;
    switch (funcName) {
        case "renameInfoSucceeded":
            if (args[6]!) {
                prefArg = args[6]!;
            }
            break;
        case "renameInfoFailed":
            if (args[1]!) {
                prefArg = args[1]!;
            }
            break;
    }
    if (prefArg) {
        if (!ts.isObjectLiteralExpression(prefArg)) {
            throw new Error(`Expected object literal expression for preferences, got ${prefArg.getText()}`);
        }
        const parsedPreferences = parseUserPreferences(prefArg);
        preferences = parsedPreferences;
    }
    return [{ kind: funcName, preferences }];
}

function parseGetEditsForFileRename(args: readonly ts.Expression[]): [VerifyGetEditsForFileRenameCmd] {
    if (args.length !== 1 || !ts.isObjectLiteralExpression(args[0]!)) {
        throw new Error(`Expected a single object literal argument in verify.getEditsForFileRename, got ${args.map(arg => arg.getText()).join(", ")}`);
    }

    let oldPath: string | undefined;
    let newPath: string | undefined;
    let newFileContents: RenameFileContent[] = [];
    let preferences = "nil /*preferences*/";

    for (const prop of args[0]!.properties) {
        if (!ts.isPropertyAssignment(prop)) {
            throw new Error(`Expected property assignment in verify.getEditsForFileRename argument, got ${prop.getText()}`);
        }
        const name = prop.name.getText();
        switch (name) {
            case "oldPath": {
                const value = getStringLiteralLike(prop.initializer);
                if (!value) {
                    throw new Error(`Expected string literal for oldPath, got ${prop.initializer.getText()}`);
                }
                oldPath = value.text;
                break;
            }
            case "newPath": {
                const value = getStringLiteralLike(prop.initializer);
                if (!value) {
                    throw new Error(`Expected string literal for newPath, got ${prop.initializer.getText()}`);
                }
                newPath = value.text;
                break;
            }
            case "newFileContents": {
                const obj = getObjectLiteralExpression(prop.initializer);
                if (!obj) {
                    throw new Error(`Expected object literal for newFileContents, got ${prop.initializer.getText()}`);
                }
                const entries: RenameFileContent[] = [];
                for (const entry of obj.properties) {
                    if (!ts.isPropertyAssignment(entry)) {
                        throw new Error(`Expected property assignment in verify.getEditsForFileRename argument, got ${prop.getText()}`);
                    }
                    const key = getStringLiteralLike(entry.name);
                    const value = getStringLiteralLike(entry.initializer);
                    if (!key || !value) {
                        throw new Error(`Expected string literal key/value in newFileContents, got ${entry.getText()}`);
                    }
                    entries.push({ path: key.text, content: value.text });
                }
                newFileContents = entries;
                break;
            }
            case "preferences": {
                if (!ts.isObjectLiteralExpression(prop.initializer)) {
                    throw new Error(`Expected object literal for preferences, got ${prop.initializer.getText()}`);
                }
                preferences = parseUserPreferences(prop.initializer);
                break;
            }
        }
    }

    if (!oldPath || !newPath) {
        throw new Error(`Expected oldPath and newPath in verify.getEditsForFileRename`);
    }

    return [{
        kind: "verifyGetEditsForFileRename",
        oldPath,
        newPath,
        newFileContents,
        preferences,
    }];
}

function parseBaselineRenameArgs(funcName: string, args: readonly ts.Expression[]): [VerifyBaselineRenameCmd] {
    let newArgs: string[] = [];
    let preferences: string | undefined;
    for (const arg of args) {
        let typedArg;
        if ((typedArg = getArrayLiteralExpression(arg))) {
            for (const elem of typedArg.elements) {
                const newArg = parseBaselineMarkerOrRangeArg(elem);
                newArgs.push(newArg);
            }
        }
        else if (ts.isObjectLiteralExpression(arg)) {
            preferences = parseUserPreferences(arg);
            continue;
        }
        else {
            newArgs.push(parseBaselineMarkerOrRangeArg(arg));
        }
    }
    return [{
        kind: funcName === "baselineRenameAtRangesWithText" ? "verifyBaselineRenameAtRangesWithText" : "verifyBaselineRename",
        args: newArgs,
        preferences: preferences ? preferences : "nil /*preferences*/",
    }];
}

function parseBaselineInlayHints(args: readonly ts.Expression[]): [VerifyBaselineInlayHintsCmd] {
    let preferences: string | undefined;
    // Parse span
    if (args.length > 0) {
        if (args[0]!.getText() !== "undefined") {
            throw new Error(`Unsupported span argument in verify.baselineInlayHints: ${args[0]!.getText()}`);
        }
    }
    // Parse preferences
    if (args.length > 1) {
        if (ts.isObjectLiteralExpression(args[1]!)) {
            preferences = parseUserPreferences(args[1]!);
        }
    }
    return [{
        kind: "verifyBaselineInlayHints",
        span: "nil /*span*/", // Only supporteed manually
        preferences: preferences ? preferences : "nil /*preferences*/",
    }];
}

function parseVerifyLinkedEditing(args: readonly ts.Expression[]): [VerifyLinkedEditingCmd] {
    var ranges = "map[string][]lsproto.Range" + args[0]!.getText().replaceAll("undefined", "nil");
    return [{
        kind: "verifyLinkedEditing",
        ranges,
    }];
}

function parseVerifyDiagnostics(funcName: string, args: readonly ts.Expression[]): [VerifyDiagnosticsCmd] {
    if (!args[0]! || !ts.isArrayLiteralExpression(args[0]!)) {
        throw new Error(`Expected an array literal argument in verify.${funcName}`);
    }
    const goArgs: string[] = [];
    for (const expr of args[0]!.elements) {
        const diag = parseExpectedDiagnostic(expr);
        goArgs.push(diag);
    }
    return [{
        kind: "verifyDiagnostics",
        arg: goArgs.length > 0 ? `[]*lsproto.Diagnostic{\n${goArgs.join(",\n")},\n}` : "nil",
        isSuggestion: funcName === "getSuggestionDiagnostics",
    }];
}

function parseExpectedDiagnostic(expr: ts.Expression): string {
    if (!ts.isObjectLiteralExpression(expr)) {
        throw new Error(`Expected object literal expression for expected diagnostic, got ${expr.getText()}`);
    }

    const diagnosticProps: string[] = [];

    for (const prop of expr.properties) {
        if (!ts.isPropertyAssignment(prop) || !(ts.isIdentifier(prop.name) || ts.isStringLiteral(prop.name))) {
            throw new Error(`Expected property assignment with identifier name for expected diagnostic, got ${prop.getText()}`);
        }

        const propName = prop.name.text;
        const init = prop.initializer;

        switch (propName) {
            case "message": {
                let messageInit;
                if (messageInit = getStringLiteralLike(init)) {
                    messageInit.text = messageInit.text.replace("/tests/cases/fourslash", "");
                    diagnosticProps.push(`Message: ${getGoStringLiteral(messageInit.text)},`);
                }
                else {
                    throw new Error(`Expected string literal for diagnostic message, got ${init.getText()}`);
                }
                break;
            }
            case "code": {
                let codeInit;
                if (codeInit = getNumericLiteral(init)) {
                    diagnosticProps.push(`Code: &lsproto.IntegerOrString{Integer: new(int32(${codeInit.text}))},`);
                }
                else {
                    throw new Error(`Expected numeric literal for diagnostic code, got ${init.getText()}`);
                }
                break;
            }
            case "range": {
                // Handle range references like ranges[0]
                const rangeArg = parseBaselineMarkerOrRangeArg(init);
                diagnosticProps.push(`Range: ${rangeArg}.LSRange,`);
                break;
            }
            case "reportsDeprecated": {
                if (init.kind === ts.SyntaxKind.TrueKeyword) {
                    diagnosticProps.push(`Tags: &[]lsproto.DiagnosticTag{lsproto.DiagnosticTagDeprecated},`);
                }
                break;
            }
            case "reportsUnnecessary": {
                if (init.kind === ts.SyntaxKind.TrueKeyword) {
                    diagnosticProps.push(`Tags: &[]lsproto.DiagnosticTag{lsproto.DiagnosticTagUnnecessary},`);
                }
                break;
            }
            default:
                throw new Error(`Unrecognized property in expected diagnostic: ${propName}`);
        }
    }

    if (diagnosticProps.length === 0) {
        throw new Error(`No valid properties found in diagnostic object`);
    }

    return `&lsproto.Diagnostic{\n${diagnosticProps.join("\n")}\n}`;
}

function parseNumberOfErrorsInCurrentFile(args: readonly ts.Expression[]): [VerifyNumberOfErrorsInCurrentFileCmd] {
    let arg0;
    if (args.length !== 1 || !(arg0 = getNumericLiteral(args[0]!))) {
        throw new Error(`Expected a single numeric literal argument in verify.numberOfErrorsInCurrentFile, got ${args.map(arg => arg.getText()).join(", ")}`);
    }
    return [{
        kind: "verifyNumberOfErrorsInCurrentFile",
        expectedCount: parseInt(arg0.text, 10),
    }];
}

function parseErrorExistsAtRange(args: readonly ts.Expression[]): [VerifyErrorExistsAtRangeCmd] {
    if (args.length < 2 || args.length > 3) {
        throw new Error(`Expected 2 or 3 arguments in verify.errorExistsAtRange, got ${args.length}`);
    }

    // First arg is a range
    const rangeArg = parseBaselineMarkerOrRangeArg(args[0]!);

    // Second arg is error code
    let codeArg;
    if (!(codeArg = getNumericLiteral(args[1]!))) {
        throw new Error(`Expected numeric literal for code in verify.errorExistsAtRange, got ${args[1]!.getText()}`);
    }

    // Third arg is optional message
    let message = "";
    if (args[2]!) {
        const messageArg = getStringLiteralLike(args[2]!);
        if (!messageArg) {
            throw new Error(`Expected string literal for message in verify.errorExistsAtRange, got ${args[2]!.getText()}`);
        }
        message = messageArg.text;
    }

    return [{
        kind: "verifyErrorExistsAtRange",
        range: rangeArg,
        code: parseInt(codeArg.text, 10),
        message: message,
    }];
}

function parseCurrentLineContentIs(args: readonly ts.Expression[]): [VerifyCurrentLineContentIsCmd] {
    let arg0;
    if (args.length !== 1 || !(arg0 = getStringLiteralLike(args[0]!))) {
        throw new Error(`Expected a single string literal argument in verify.currentLineContentIs, got ${args.map(arg => arg.getText()).join(", ")}`);
    }
    return [{
        kind: "verifyCurrentLineContentIs",
        text: arg0.text,
    }];
}

function parseCurrentFileContentIs(args: readonly ts.Expression[]): [VerifyCurrentFileContentIsCmd] {
    let arg0;
    if (args.length !== 1 || !(arg0 = getStringLiteralLike(args[0]!))) {
        throw new Error(`Expected a single string literal argument in verify.currentFileContentIs, got ${args.map(arg => arg.getText()).join(", ")}`);
    }
    return [{
        kind: "verifyCurrentFileContentIs",
        text: arg0.text,
    }];
}

function parseErrorExistsBetweenMarkers(args: readonly ts.Expression[]): [VerifyErrorExistsBetweenMarkersCmd] {
    if (args.length !== 2) {
        throw new Error(`Expected 2 arguments in verify.errorExistsBetweenMarkers, got ${args.length}`);
    }
    let startMarker, endMarker;
    if (!(startMarker = getStringLiteralLike(args[0]!)) || !(endMarker = getStringLiteralLike(args[1]!))) {
        throw new Error(`Expected string literal arguments in verify.errorExistsBetweenMarkers, got ${args.map(arg => arg.getText()).join(", ")}`);
    }
    return [{
        kind: "verifyErrorExistsBetweenMarkers",
        startMarker: startMarker.text,
        endMarker: endMarker.text,
    }];
}

function parseErrorExistsAfterMarker(args: readonly ts.Expression[]): [VerifyErrorExistsAfterMarkerCmd] {
    let markerName = "";
    if (args.length > 0) {
        const arg0 = getStringLiteralLike(args[0]!);
        if (!arg0) {
            throw new Error(`Expected string literal argument in verify.errorExistsAfterMarker, got ${args[0]!.getText()}`);
        }
        markerName = arg0.text;
    }
    return [{
        kind: "verifyErrorExistsAfterMarker",
        markerName: markerName,
    }];
}

function parseErrorExistsBeforeMarker(args: readonly ts.Expression[]): [VerifyErrorExistsBeforeMarkerCmd] {
    let markerName = "";
    if (args.length > 0) {
        const arg0 = getStringLiteralLike(args[0]!);
        if (!arg0) {
            throw new Error(`Expected string literal argument in verify.errorExistsBeforeMarker, got ${args[0]!.getText()}`);
        }
        markerName = arg0.text;
    }
    return [{
        kind: "verifyErrorExistsBeforeMarker",
        markerName: markerName,
    }];
}

function parseCodeFixArgs(args: readonly ts.Expression[]): [VerifyCodeFixCmd] {
    if (args.length !== 1) {
        throw new Error(`Expected 1 argument in verify.codeFix, got ${args.length}`);
    }
    const obj = getObjectLiteralExpression(args[0]!);
    if (!obj) {
        throw new Error(`Expected object literal in verify.codeFix, got ${args[0]!.getText()}`);
    }

    const sourceFile = args[0]!.getSourceFile();
    let description = "";
    let newFileContent: string | undefined;
    let newRangeContent: string | undefined;
    let index = 0;
    let applyChanges = false;
    let preferences = "nil /*preferences*/";

    for (const prop of obj.properties) {
        const name = getPropertyName(prop);
        if (!name) continue;
        if (ts.isShorthandPropertyAssignment(prop)) {
            if (name === "description") {
                const resolved = resolveDescriptionExpression(prop.name, sourceFile);
                if (resolved) description = resolved;
            }
            continue;
        }
        if (!ts.isPropertyAssignment(prop)) continue;
        switch (name) {
            case "description": {
                const resolved = resolveDescriptionExpression(prop.initializer, sourceFile);
                if (resolved) description = resolved;
                break;
            }
            case "newFileContent": {
                const str = getStringLiteralLike(prop.initializer);
                if (str) newFileContent = str.text;
                break;
            }
            case "newRangeContent": {
                const str = getStringLiteralLike(prop.initializer);
                if (str) newRangeContent = str.text;
                break;
            }
            case "index": {
                const num = getNumericLiteral(prop.initializer);
                if (num) index = parseInt(num.text);
                break;
            }
            case "applyChanges": {
                if (prop.initializer.kind === ts.SyntaxKind.TrueKeyword) {
                    applyChanges = true;
                }
                break;
            }
            case "preferences": {
                const prefs = getObjectLiteralExpression(prop.initializer);
                if (!prefs) {
                    throw new Error(`Expected object literal for preferences in verify.codeFix, got ${prop.initializer.getText()}`);
                }
                preferences = parseUserPreferences(prefs);
                break;
            }
        }
    }

    return [{
        kind: "verifyCodeFix",
        description,
        newFileContent,
        newRangeContent,
        index,
        applyChanges,
        preferences,
    }];
}

function parseCodeFixAvailableArgs(funcName: string, args: readonly ts.Expression[]): [VerifyCodeFixAvailableCmd] {
    switch (funcName) {
        case "codeFixAvailable": {
            const descriptions: string[] = [];
            let expectNone = false;

            if (args.length === 1) {
                const sourceFile = args[0]!.getSourceFile();
                const arrayArg = getArrayLiteralExpression(args[0]!);
                if (arrayArg) {
                    if (arrayArg.elements.length === 0) {
                        expectNone = true;
                    }
                    for (const elem of arrayArg.elements) {
                        const obj = getObjectLiteralExpression(elem);
                        if (obj) {
                            for (const prop of obj.properties) {
                                if (getPropertyName(prop) === "description") {
                                    let resolved: string | undefined;
                                    if (ts.isPropertyAssignment(prop)) {
                                        resolved = resolveDescriptionExpression(prop.initializer, sourceFile);
                                    }
                                    else if (ts.isShorthandPropertyAssignment(prop)) {
                                        resolved = resolveDescriptionExpression(prop.name, sourceFile);
                                    }
                                    if (resolved) {
                                        descriptions.push(resolved);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return [{
                kind: "verifyCodeFixAvailable",
                descriptions,
                unavailableDescriptions: [],
                expectNone,
            }];
        }
        case "notCodeFixAvailable":
            if (args.length === 0) {
                return [{
                    kind: "verifyCodeFixAvailable",
                    descriptions: [],
                    unavailableDescriptions: [],
                    expectNone: true,
                }];
            }
            if (args.length === 1) {
                const descriptionExpression = args[0]!;
                const description = resolveDescriptionExpression(descriptionExpression, descriptionExpression.getSourceFile());
                if (!description) {
                    throw new Error(`Unsupported argument in verify.not.codeFixAvailable: ${descriptionExpression.getText()}`);
                }
                return [{
                    kind: "verifyCodeFixAvailable",
                    descriptions: [],
                    unavailableDescriptions: [description],
                    expectNone: false,
                }];
            }
            throw new Error(`Expected 0 or 1 arguments in verify.not.codeFixAvailable, got ${args.map(arg => arg.getText()).join(", ")}`);
        default:
            throw new Error(`Unrecognized codeFixAvailable function: ${funcName}`);
    }
}

function parseRangeAfterCodeFixArgs(args: readonly ts.Expression[]): [VerifyRangeAfterCodeFixCmd] {
    const [expectedTextArg, includeWhiteSpaceArg, errorCodeArg, indexArg] = args;
    const expectedText = expectedTextArg && getStringLiteralLike(expectedTextArg);
    if (!expectedText) {
        throw new Error(`Expected string literal argument in verify.rangeAfterCodeFix, got ${expectedTextArg?.getText()}`);
    }

    let includeWhiteSpace = false;
    if (includeWhiteSpaceArg !== undefined && !isUndefinedExpression(includeWhiteSpaceArg)) {
        includeWhiteSpace = includeWhiteSpaceArg.kind === ts.SyntaxKind.TrueKeyword;
    }

    let errorCode = 0;
    if (errorCodeArg !== undefined) {
        const parsedErrorCode = getNumericLiteral(errorCodeArg);
        if (!parsedErrorCode) {
            throw new SkipTest(`Expected numeric literal errorCode in verify.rangeAfterCodeFix, got ${errorCodeArg.getText()}`);
        }
        errorCode = parseInt(parsedErrorCode.text);
    }

    let index = 0;
    if (indexArg !== undefined) {
        const parsedIndex = getNumericLiteral(indexArg);
        if (!parsedIndex) {
            throw new SkipTest(`Expected numeric literal index in verify.rangeAfterCodeFix, got ${indexArg.getText()}`);
        }
        index = parseInt(parsedIndex.text);
    }

    return [{
        kind: "verifyRangeAfterCodeFix",
        expectedText: expectedText.text,
        includeWhiteSpace,
        errorCode,
        index,
    }];
}

function parseCodeFixAllArgs(args: readonly ts.Expression[]): [VerifyCodeFixAllCmd] {
    if (args.length !== 1) {
        throw new Error(`Expected 1 argument in verify.codeFixAll, got ${args.length}`);
    }
    const obj = getObjectLiteralExpression(args[0]!);
    if (!obj) {
        throw new Error(`Expected object literal in verify.codeFixAll, got ${args[0]!.getText()}`);
    }

    let fixId = "";
    let newFileContent = "";

    for (const prop of obj.properties) {
        if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) continue;
        switch (prop.name.text) {
            case "fixId": {
                const str = getStringLiteralLike(prop.initializer);
                if (str) fixId = str.text;
                break;
            }
            case "newFileContent": {
                const str = getStringLiteralLike(prop.initializer);
                if (str) newFileContent = str.text;
                break;
            }
        }
    }

    return [{
        kind: "verifyCodeFixAll",
        fixId,
        newFileContent,
    }];
}

function stringToTristate(s: string): string {
    switch (s) {
        case "true":
            return "core.TSTrue";
        case "false":
            return "core.TSFalse";
        default:
            return "core.TSUnknown";
    }
}

function parseCodeFixAllAvailableArgs(args: readonly ts.Expression[]): [VerifyCodeFixAllAvailableCmd] {
    if (args.length !== 1) {
        throw new Error(`Expected 1 argument in verify.not.codeFixAllAvailable, got ${args.length}`);
    }
    const fixId = getStringLiteralLike(args[0]!);
    if (!fixId) {
        throw new Error(`Expected string literal in verify.not.codeFixAllAvailable, got ${args[0]!.getText()}`);
    }
    return [{
        kind: "verifyCodeFixAllNotAvailable",
        fixId: fixId.text,
    }];
}

function rustQuotePreference(value: string): string {
    switch (value) {
        case "auto":
            return "lsutil.QuotePreference::Auto";
        case "double":
            return "lsutil.QuotePreference::Double";
        case "single":
            return "lsutil.QuotePreference::Single";
        default:
            return "lsutil.QuotePreference::Unknown";
    }
}

function rustImportModuleSpecifierPreference(value: string): string {
    switch (value) {
        case "shortest":
            return "modulespecifiers::ImportModuleSpecifierPreference::Shortest";
        case "project-relative":
            return "modulespecifiers::ImportModuleSpecifierPreference::ProjectRelative";
        case "relative":
            return "modulespecifiers::ImportModuleSpecifierPreference::Relative";
        case "non-relative":
            return "modulespecifiers::ImportModuleSpecifierPreference::NonRelative";
        default:
            return "modulespecifiers::ImportModuleSpecifierPreference::None";
    }
}

function rustImportModuleSpecifierEndingPreference(value: string): string {
    switch (value) {
        case "auto":
            return "modulespecifiers::ImportModuleSpecifierEndingPreference::Auto";
        case "minimal":
            return "modulespecifiers::ImportModuleSpecifierEndingPreference::Minimal";
        case "index":
            return "modulespecifiers::ImportModuleSpecifierEndingPreference::Index";
        case "js":
            return "modulespecifiers::ImportModuleSpecifierEndingPreference::Js";
        default:
            return "modulespecifiers::ImportModuleSpecifierEndingPreference::None";
    }
}

function parseUserPreferences(arg: ts.ObjectLiteralExpression): string {
    const inlayHintPreferences: string[] = [];
    const moduleSpecifierPreferences: string[] = [];
    const preferences: string[] = [];
    for (const prop of arg.properties) {
        if (ts.isPropertyAssignment(prop)) {
            switch (prop.name.getText()) {
                // !!! other preferences
                case "providePrefixAndSuffixTextForRename":
                    preferences.push(`UseAliasesForRename: ${stringToTristate(prop.initializer.getText())}`);
                    break;
                case "quotePreference":
                    if (!ts.isStringLiteralLike(prop.initializer)) {
                        throw new Error(`Expected string literal for quotePreference, got ${prop.initializer.getText()}`);
                    }
                    preferences.push(`QuotePreference: ${rustQuotePreference(prop.initializer.text)}`);
                    break;
                case "autoImportSpecifierExcludeRegexes":
                    const regexArrayArg = getArrayLiteralExpression(prop.initializer);
                    if (!regexArrayArg) {
                        throw new Error(`Expected array literal for autoImportSpecifierExcludeRegexes, got ${prop.initializer.getText()}`);
                    }
                    const regexes: string[] = [];
                    for (const elem of regexArrayArg.elements) {
                        const strElem = getStringLiteralLike(elem);
                        if (!strElem) {
                            throw new Error(`Expected string literal in autoImportSpecifierExcludeRegexes array, got ${elem.getText()}`);
                        }
                        regexes.push(`${rustStringLiteral(strElem.text)}.to_string()`);
                    }
                    moduleSpecifierPreferences.push(`AutoImportSpecifierExcludeRegexes: vec![${regexes.join(", ")}]`);
                    break;
                case "importModuleSpecifierPreference":
                    if (!ts.isStringLiteralLike(prop.initializer)) {
                        throw new Error(`Expected string literal for importModuleSpecifierPreference, got ${prop.initializer.getText()}`);
                    }
                    moduleSpecifierPreferences.push(`ImportModuleSpecifierPreference: ${rustImportModuleSpecifierPreference(prop.initializer.text)}`);
                    break;
                case "importModuleSpecifierEnding":
                    if (!ts.isStringLiteralLike(prop.initializer)) {
                        throw new Error(`Expected string literal for importModuleSpecifierEnding, got ${prop.initializer.getText()}`);
                    }
                    moduleSpecifierPreferences.push(`ImportModuleSpecifierEnding: ${rustImportModuleSpecifierEndingPreference(prop.initializer.text)}`);
                    break;
                case "allowRenameOfImportPath":
                    preferences.push(`AllowRenameOfImportPath: ${stringToTristate(prop.initializer.getText())}`);
                    break;
                case "preferTypeOnlyAutoImports":
                    preferences.push(`PreferTypeOnlyAutoImports: ${stringToTristate(prop.initializer.getText())}`);
                    break;
                case "organizeImportsTypeOrder":
                    if (!ts.isStringLiteralLike(prop.initializer)) {
                        throw new Error(`Expected string literal for organizeImportsTypeOrder, got ${prop.initializer.getText()}`);
                    }
                    switch (prop.initializer.text) {
                        case "last":
                            preferences.push(`OrganizeImportsTypeOrder: lsutil.OrganizeImportsTypeOrderLast`);
                            break;
                        case "inline":
                            preferences.push(`OrganizeImportsTypeOrder: lsutil.OrganizeImportsTypeOrderInline`);
                            break;
                        case "first":
                            preferences.push(`OrganizeImportsTypeOrder: lsutil.OrganizeImportsTypeOrderFirst`);
                            break;
                        default:
                            throw new Error(`Unsupported organizeImportsTypeOrder value: ${prop.initializer.text}`);
                    }
                    break;
                case "autoImportFileExcludePatterns":
                    const arrayArg = getArrayLiteralExpression(prop.initializer);
                    if (!arrayArg) {
                        throw new Error(`Expected array literal for autoImportFileExcludePatterns, got ${prop.initializer.getText()}`);
                    }
                    const patterns: string[] = [];
                    for (const elem of arrayArg.elements) {
                        const strElem = getStringLiteralLike(elem);
                        if (!strElem) {
                            throw new Error(`Expected string literal in autoImportFileExcludePatterns array, got ${elem.getText()}`);
                        }
                        patterns.push(`${rustStringLiteral(strElem.text)}.to_string()`);
                    }
                    preferences.push(`AutoImportFileExcludePatterns: vec![${patterns.join(", ")}]`);
                    break;
                case "includeInlayParameterNameHints":
                    let paramHint;
                    if (!ts.isStringLiteralLike(prop.initializer)) {
                        throw new Error(`Expected string literal for includeInlayParameterNameHints, got ${prop.initializer.getText()}`);
                    }
                    switch (prop.initializer.text) {
                        case "none":
                            paramHint = `Some("none".to_string())`;
                            break;
                        case "literals":
                            paramHint = `Some("literals".to_string())`;
                            break;
                        case "all":
                            paramHint = `Some("all".to_string())`;
                            break;
                    }
                    inlayHintPreferences.push(`IncludeInlayParameterNameHints: ${paramHint}`);
                    break;
                case "includeInlayParameterNameHintsWhenArgumentMatchesName":
                    inlayHintPreferences.push(`IncludeInlayParameterNameHintsWhenArgumentMatchesName: ${stringToTristate(prop.initializer.getText())}`);
                    break;
                case "includeInlayFunctionParameterTypeHints":
                    inlayHintPreferences.push(`IncludeInlayFunctionParameterTypeHints: ${stringToTristate(prop.initializer.getText())}`);
                    break;
                case "includeInlayVariableTypeHints":
                    inlayHintPreferences.push(`IncludeInlayVariableTypeHints: ${stringToTristate(prop.initializer.getText())}`);
                    break;
                case "includeInlayVariableTypeHintsWhenTypeMatchesName":
                    inlayHintPreferences.push(`IncludeInlayVariableTypeHintsWhenTypeMatchesName: ${stringToTristate(prop.initializer.getText())}`);
                    break;
                case "includeInlayPropertyDeclarationTypeHints":
                    inlayHintPreferences.push(`IncludeInlayPropertyDeclarationTypeHints: ${stringToTristate(prop.initializer.getText())}`);
                    break;
                case "includeInlayFunctionLikeReturnTypeHints":
                    inlayHintPreferences.push(`IncludeInlayFunctionLikeReturnTypeHints: ${stringToTristate(prop.initializer.getText())}`);
                    break;
                case "includeInlayEnumMemberValueHints":
                    inlayHintPreferences.push(`IncludeInlayEnumMemberValueHints: ${stringToTristate(prop.initializer.getText())}`);
                    break;
                case "interactiveInlayHints":
                    // Ignore, deprecated
                    break;
            }
        }
        else if (ts.isShorthandPropertyAssignment(prop)) {
            const init = getInitializer(prop.name);
            switch (prop.name.text) {
                case "autoImportFileExcludePatterns": {
                    const patterns = rustStringArrayFromExpression(init);
                    preferences.push(`AutoImportFileExcludePatterns: vec![${patterns.map(pattern => `${rustStringLiteral(pattern)}.to_string()`).join(", ")}]`);
                    break;
                }
                case "autoImportSpecifierExcludeRegexes": {
                    const regexes = rustStringArrayFromExpression(init);
                    moduleSpecifierPreferences.push(`AutoImportSpecifierExcludeRegexes: vec![${regexes.map(regex => `${rustStringLiteral(regex)}.to_string()`).join(", ")}]`);
                    break;
                }
                default:
                    throw new Error(`Expected property assignment in user preferences object, got ${prop.getText()}`);
            }
        }
        else {
            throw new Error(`Expected property assignment in user preferences object, got ${prop.getText()}`);
        }
    }

    if (inlayHintPreferences.length > 0) {
        preferences.push(`InlayHints: lsutil.InlayHintsPreferences{${inlayHintPreferences.join(",")}}`);
    }
    if (moduleSpecifierPreferences.length > 0) {
        preferences.push(...moduleSpecifierPreferences);
    }
    if (preferences.length === 0) {
        return "nil /*preferences*/";
    }
    return `UserPreferences{${preferences.join(",")}, ..Default::default()}`;
}

function rustStringArrayFromExpression(expr: ts.Expression | undefined): string[] {
    const arrayArg = expr && getArrayLiteralExpression(expr);
    if (!arrayArg) {
        throw new Error(`Expected string array expression, got ${expr?.getText()}`);
    }
    return arrayArg.elements.map(elem => {
        const strElem = getStringLiteralLike(elem);
        if (!strElem) {
            throw new Error(`Expected string literal in string array, got ${elem.getText()}`);
        }
        return strElem.text;
    });
}

function parseBaselineMarkerOrRangeArg(arg: ts.Expression): string {
    if (ts.isStringLiteral(arg)) {
        return getGoStringLiteral(arg.text);
    }
    else if (ts.isIdentifier(arg) || (ts.isElementAccessExpression(arg) && ts.isIdentifier(arg.expression))) {
        const result = parseRangeVariable(arg);
        if (result) {
            return result;
        }
        const init = getNodeOfKind(arg, ts.isCallExpression);
        if (init) {
            const result = getRangesByTextArg(init);
            if (result) {
                return result;
            }
        }
    }
    else if (ts.isElementAccessExpression(arg) && ts.isCallExpression(arg.expression) && arg.expression.getText().includes("ranges")) {
        // `test.ranges()[n]`
        const index = arg.argumentExpression?.getText();
        if (index !== undefined) {
            return `f.Ranges()[${index}]`;
        }
    }
    else if (ts.isCallExpression(arg)) {
        const result = getRangesByTextArg(arg);
        if (result) {
            return result;
        }
        // Handle `.filter(r => !(r.marker && r.marker.data.KEY))` patterns
        const filterResult = parseFilterExpression(arg);
        if (filterResult) {
            return filterResult;
        }
    }
    if (arg.getText() === "test.markers()") {
        return "ToAny(f.Markers())...";
    }
    throw new Error(`Unrecognized marker or range argument: ${arg.getText()}`);
}

function parseRangeVariable(arg: ts.Identifier | ts.ElementAccessExpression): string | undefined {
    const argName = ts.isIdentifier(arg) ? arg.text : (arg.expression as ts.Identifier).text;
    const file = arg.getSourceFile();
    const varStmts = file.statements.filter(ts.isVariableStatement);
    for (const varStmt of varStmts) {
        for (const decl of varStmt.declarationList.declarations) {
            if (ts.isArrayBindingPattern(decl.name) && decl.initializer) {
                // Resolve the initializer to a Go expression for the source array
                const sourceExpr = resolveRangesExpression(decl.initializer, varStmts);
                if (!sourceExpr) continue;
                for (let i = 0; i < decl.name.elements.length; i++) {
                    const elem = decl.name.elements[i]!;
                    if (ts.isBindingElement(elem) && ts.isIdentifier(elem.name) && elem.name.text === argName) {
                        if (elem.dotDotDotToken === undefined) {
                            return `${sourceExpr}[${i}]`;
                        }
                        if (ts.isElementAccessExpression(arg)) {
                            return `${sourceExpr}[${i + parseInt(arg.argumentExpression!.getText())}]`;
                        }
                        return `ToAny(${sourceExpr}[${i}:])...`;
                    }
                }
            }
            // `const ranges = test.ranges();` and arg is `ranges[n]`
            if (ts.isIdentifier(decl.name) && decl.name.text === argName && decl.initializer?.getText().includes("ranges")) {
                if (ts.isElementAccessExpression(arg)) {
                    return `f.Ranges()[${arg.argumentExpression!.getText()}]`;
                }
                // `const range = test.ranges()[0]` used directly as `range`
                if (ts.isIdentifier(arg) && ts.isElementAccessExpression(decl.initializer) && ts.isCallExpression(decl.initializer.expression) && decl.initializer.argumentExpression) {
                    return `f.Ranges()[${decl.initializer.argumentExpression.getText()}]`;
                }
            }
            // `const cRanges = ranges.get("C")` or `const cRanges = test.rangesByText().get("C")`
            if (ts.isIdentifier(decl.name) && decl.name.text === argName && decl.initializer && ts.isCallExpression(decl.initializer)) {
                const initText = decl.initializer.getText();
                if (initText.includes("rangesByText") || (ts.isPropertyAccessExpression(decl.initializer.expression) && decl.initializer.expression.name.text === "get")) {
                    // Try to find the .get("text") argument
                    const getCall = decl.initializer;
                    if (getCall.arguments.length === 1 && ts.isStringLiteralLike(getCall.arguments[0]!)) {
                        return `ToAny(f.GetRangesByText().Get(${getGoStringLiteral(getCall.arguments[0]!.text)}))...`;
                    }
                }
            }
        }
    }
    return undefined;
}

/**
 * Resolves a range initializer expression to a Go expression.
 * Handles `test.ranges()`, `test.rangesByText().get("X")`, and variable references to those.
 */
function resolveRangesExpression(expr: ts.Expression, varStmts: ts.VariableStatement[]): string | undefined {
    const text = expr.getText();
    if (text.includes("test.ranges()")) {
        return "f.Ranges()";
    }
    if (text.includes("rangesByText()") && ts.isCallExpression(expr) && expr.arguments.length === 1 && ts.isStringLiteralLike(expr.arguments[0]!)) {
        return `f.GetRangesByText().Get(${getGoStringLiteral(expr.arguments[0]!.text)})`;
    }
    // Handle `someVar.get("text")` where someVar resolves to rangesByText()
    if (ts.isCallExpression(expr) && ts.isPropertyAccessExpression(expr.expression) && expr.expression.name.text === "get" && expr.arguments.length === 1 && ts.isStringLiteralLike(expr.arguments[0]!)) {
        const obj = expr.expression.expression;
        if (ts.isIdentifier(obj)) {
            const resolved = resolveIdentifier(obj, varStmts);
            if (resolved?.includes("rangesByText")) {
                return `f.GetRangesByText().Get(${getGoStringLiteral(expr.arguments[0]!.text)})`;
            }
        }
    }
    // It might be a variable reference like `const [d0, d1] = dRanges;`
    if (ts.isIdentifier(expr)) {
        for (const varStmt of varStmts) {
            for (const decl of varStmt.declarationList.declarations) {
                if (ts.isIdentifier(decl.name) && decl.name.text === expr.text && decl.initializer) {
                    return resolveRangesExpression(decl.initializer, varStmts);
                }
            }
        }
    }
    return undefined;
}

function resolveIdentifier(id: ts.Identifier, varStmts: ts.VariableStatement[]): string | undefined {
    for (const varStmt of varStmts) {
        for (const decl of varStmt.declarationList.declarations) {
            if (ts.isIdentifier(decl.name) && decl.name.text === id.text && decl.initializer) {
                return decl.initializer.getText();
            }
        }
    }
    return undefined;
}

function getRangesByTextArg(arg: ts.CallExpression): string | undefined {
    if (arg.getText().startsWith("test.rangesByText()")) {
        if (ts.isStringLiteralLike(arg.arguments[0]!)) {
            return `ToAny(f.GetRangesByText().Get(${getGoStringLiteral(arg.arguments[0]!.text)}))...`;
        }
    }
    return undefined;
}

/**
 * Handles `.filter(r => !(r.marker && r.marker.data.KEY))` patterns on range arrays.
 * Converts to `ToAny(core.Filter(source, func(r *fourslash.RangeMarker) bool { ... }))...`
 */
function parseFilterExpression(arg: ts.CallExpression): string | undefined {
    if (!ts.isPropertyAccessExpression(arg.expression) || arg.expression.name.text !== "filter") {
        return undefined;
    }
    if (arg.arguments.length !== 1) {
        return undefined;
    }
    const filterArg = arg.arguments[0]!;
    if (!ts.isArrowFunction(filterArg)) {
        return undefined;
    }

    // Resolve the source (the thing being filtered)
    const sourceExpr = arg.expression.expression;
    let sourceGo: string | undefined;

    if (ts.isIdentifier(sourceExpr)) {
        const file = sourceExpr.getSourceFile();
        const varStmts = file.statements.filter(ts.isVariableStatement);
        sourceGo = resolveRangesExpression(sourceExpr, varStmts);
    }
    else if (ts.isCallExpression(sourceExpr)) {
        // e.g. test.ranges().filter(...)
        if (sourceExpr.getText().includes("test.ranges()")) {
            sourceGo = "f.Ranges()";
        }
    }

    if (!sourceGo) {
        return undefined;
    }

    // Parse the filter body to generate a Go predicate
    const predicate = parseFilterPredicate(filterArg);
    if (!predicate) {
        return undefined;
    }

    return `ToAny(core.Filter(${sourceGo}, ${predicate}))...`;
}

function parseFilterPredicate(arrow: ts.ArrowFunction): string | undefined {
    // Handle `r => !(r.marker && r.marker.data.KEY)` → filter OUT ranges with marker.data.KEY
    const body = arrow.body;
    if (!ts.isExpression(body)) {
        return undefined;
    }

    const paramName = arrow.parameters[0]?.name.getText();
    if (!paramName) {
        return undefined;
    }

    // Check for `!(r.marker && r.marker.data.KEY)` pattern
    if (ts.isPrefixUnaryExpression(body) && body.operator === ts.SyntaxKind.ExclamationToken) {
        const inner = ts.isParenthesizedExpression(body.operand) ? body.operand.expression : body.operand;
        if (ts.isBinaryExpression(inner) && inner.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken) {
            // Right side should be `r.marker.data.KEY`
            const right = inner.right;
            if (ts.isPropertyAccessExpression(right) && right.expression.getText() === `${paramName}.marker.data`) {
                const key = right.name.text;
                return `func(r *fourslash.RangeMarker) bool { return r.Marker == nil || r.Marker.Data[${getGoStringLiteral(key)}] == nil }`;
            }
        }
    }

    return undefined;
}

function parseBaselineQuickInfo(args: ts.NodeArray<ts.Expression>): VerifyBaselineQuickInfoCmd[] {
    if (args.length === 0) {
        return [{
            kind: "verifyBaselineQuickInfo",
        }];
    }
    // First arg is verbosityLevels: { markerName: number | number[] }
    const verbosityArg = args[0]!;
    if (!ts.isObjectLiteralExpression(verbosityArg)) {
        throw new Error(`Expected object literal expression for verify.baselineQuickInfo argument, got ${verbosityArg.getText()}`);
    }
    const verbosityLevels: Record<string, number[]> = {};
    for (const prop of verbosityArg.properties) {
        if (!ts.isPropertyAssignment(prop)) {
            throw new Error(`Expected property assignment in baselineQuickInfo verbosity levels, got ${prop.getText()}`);
        }
        let name: string;
        if (ts.isIdentifier(prop.name) || ts.isStringLiteral(prop.name)) {
            name = prop.name.text;
        }
        else if (ts.isNumericLiteral(prop.name)) {
            name = prop.name.text;
        }
        else {
            throw new Error(`Expected identifier, string, or numeric literal for property name in baselineQuickInfo verbosity levels, got ${prop.name.getText()}`);
        }
        if (ts.isArrayLiteralExpression(prop.initializer)) {
            const levels: number[] = [];
            for (const elem of prop.initializer.elements) {
                if (!ts.isNumericLiteral(elem)) {
                    throw new Error(`Expected numeric literal in baselineQuickInfo verbosity levels array, got ${elem.getText()}`);
                }
                levels.push(Number(elem.text));
            }
            verbosityLevels[name] = levels;
        }
        else if (ts.isNumericLiteral(prop.initializer)) {
            verbosityLevels[name] = [Number(prop.initializer.text)];
        }
        else {
            throw new Error(`Expected numeric literal or array literal for baselineQuickInfo verbosity level, got ${prop.initializer.getText()}`);
        }
    }
    return [{
        kind: "verifyBaselineQuickInfo",
        verbosityLevels,
    }];
}

function parseQuickInfoArgs(funcName: string, args: readonly ts.Expression[], env: ParseEnv = {}): VerifyQuickInfoCmd[] {
    // We currently don't support 'expectedTags'.
    switch (funcName) {
        case "quickInfoAt": {
            if (args.length < 1 || args.length > 3) {
                throw new Error(`Expected 1 or 2 arguments in quickInfoIs, got ${args.map(arg => arg.getText()).join(", ")}`);
            }
            let marker: string | undefined;
            let eachMarker = false;
            if (ts.isIdentifier(args[0]!) && args[0]!.text === env.markerNamesVar) {
                eachMarker = true;
            }
            else {
                marker = getStaticStringExpression(args[0]!, env);
            }
            if (marker === undefined && !eachMarker) {
                throw new Error(`Expected string literal for first argument in quickInfoAt, got ${args[0]!.getText()}`);
            }
            let text: string | undefined;
            if (args[1]!) {
                text = getStaticStringExpression(args[1]!, env);
                if (text === undefined) {
                    throw new Error(`Expected string literal for second argument in quickInfoAt, got ${args[1]!.getText()}`);
                }
            }
            let docs: string | undefined;
            if (args[2]!) {
                docs = getStaticStringExpression(args[2]!, env);
                if (docs === undefined && args[2]!.getText() !== "undefined") {
                    throw new Error(`Expected string literal or undefined for third argument in quickInfoAt, got ${args[2]!.getText()}`);
                }
            }
            return [{
                kind: eachMarker ? "quickInfoAtEachMarker" : "quickInfoAt",
                marker,
                text,
                docs,
            }];
        }
        case "quickInfos": {
            const cmds: VerifyQuickInfoCmd[] = [];
            let arg0;
            if (args.length !== 1 || !(arg0 = getObjectLiteralExpression(args[0]!))) {
                throw new Error(`Expected a single object literal argument in quickInfos, got ${args.map(arg => arg.getText()).join(", ")}`);
            }
            for (const prop of arg0.properties) {
                if (!ts.isPropertyAssignment(prop)) {
                    throw new Error(`Expected property assignment in quickInfos, got ${prop.getText()}`);
                }
                if (!(ts.isIdentifier(prop.name) || ts.isStringLiteralLike(prop.name) || ts.isNumericLiteral(prop.name))) {
                    throw new Error(`Expected identifier or literal for property name in quickInfos, got ${prop.name.getText()}`);
                }
                const marker = prop.name.text;
                let text: string;
                let docs: string | undefined;
                let init;
                if (init = getArrayLiteralExpression(prop.initializer)) {
                    if (init.elements.length !== 2) {
                        throw new Error(`Expected two elements in array literal for quickInfos property, got ${init.getText()}`);
                    }
                    let textExp, docsExp;
                    if (!(textExp = getStringLiteralLike(init.elements[0]!)) || !(docsExp = getStringLiteralLike(init.elements[1]!))) {
                        throw new Error(`Expected string literals in array literal for quickInfos property, got ${init.getText()}`);
                    }
                    text = textExp.text;
                    docs = docsExp.text;
                }
                else if (init = getStringLiteralLike(prop.initializer)) {
                    text = init.text;
                }
                else {
                    throw new Error(`Expected string literal or array literal for quickInfos property, got ${prop.initializer.getText()}`);
                }
                cmds.push(addTrailingComments({
                    kind: "quickInfoAt",
                    marker,
                    text,
                    docs,
                }, prop));
            }
            return cmds;
        }
        case "quickInfoExists":
            return [{
                kind: "quickInfoExists",
            }];
        case "notQuickInfoExists":
            return [{
                kind: "notQuickInfoExists",
            }];
        case "quickInfoIs": {
            if (args.length < 1 || args.length > 2) {
                throw new Error(`Expected 1 or 2 arguments in quickInfoIs, got ${args.map(arg => arg.getText()).join(", ")}`);
            }
            const text = getStaticStringExpression(args[0]!, env);
            if (text === undefined) {
                throw new Error(`Expected string literal for first argument in quickInfoIs, got ${args[0]!.getText()}`);
            }
            let docs: string | undefined;
            if (args[1]!) {
                docs = getStaticStringExpression(args[1]!, env);
                if (docs === undefined) {
                    throw new Error(`Expected string literal for second argument in quickInfoIs, got ${args[1]!.getText()}`);
                }
            }
            return [{
                kind: "quickInfoIs",
                text,
                docs,
            }];
        }
    }
    throw new Error(`Unrecognized quick info function: ${funcName}`);
}

function getStaticStringExpression(expr: ts.Expression, env: ParseEnv = {}): string | undefined {
    const literal = getStringLiteralLike(expr);
    if (literal) return literal.text;
    if (ts.isNumericLiteral(expr)) return expr.text;
    if (ts.isIdentifier(expr) && env.stringVars && Object.prototype.hasOwnProperty.call(env.stringVars, expr.text)) {
        return env.stringVars[expr.text];
    }
    if (ts.isBinaryExpression(expr) && expr.operatorToken.kind === ts.SyntaxKind.PlusToken) {
        const left = getStaticStringExpression(expr.left, env);
        const right = getStaticStringExpression(expr.right, env);
        return left !== undefined && right !== undefined ? left + right : undefined;
    }

    if (
        ts.isCallExpression(expr)
        && ts.isPropertyAccessExpression(expr.expression)
        && expr.expression.name.text === "join"
        && ts.isArrayLiteralExpression(expr.expression.expression)
    ) {
        const separator = expr.arguments.length === 0
            ? ","
            : getStaticStringExpression(expr.arguments[0]!, env);
        if (separator === undefined) return undefined;
        const parts: string[] = [];
        for (const element of expr.expression.expression.elements) {
            const value = getStaticStringExpression(element, env);
            if (value === undefined) return undefined;
            parts.push(value);
        }
        return parts.join(separator);
    }

    return undefined;
}

function parseOrganizeImportsArgs(args: readonly ts.Expression[]): [VerifyOrganizeImportsCmd] {
    if (args.length < 1 || args.length > 3) {
        throw new Error(`Expected 1-3 arguments in verify.organizeImports, got ${args.length}`);
    }

    const expectedContent = getStringLiteralLike(args[0]!);
    if (!expectedContent) {
        throw new Error(`Expected string literal as first argument in verify.organizeImports, got ${args[0]!.getText()}`);
    }

    let mode = "lsproto.CodeActionKindSourceOrganizeImports";
    if (args.length >= 2 && args[1]!.getText() !== "undefined") {
        const modeExpr = args[1]!;
        if (
            ts.isPropertyAccessExpression(modeExpr) &&
            modeExpr.expression.getText() === "ts.OrganizeImportsMode"
        ) {
            const modeName = modeExpr.name.text;
            switch (modeName) {
                case "RemoveUnused":
                    mode = "lsproto.CodeActionKindSourceRemoveUnusedImports";
                    break;
                case "SortAndCombine":
                    mode = "lsproto.CodeActionKindSourceSortImports";
                    break;
                case "All":
                    mode = "lsproto.CodeActionKindSourceOrganizeImports";
                    break;
                default:
                    throw new Error(`Unsupported organize imports mode: ${modeName}`);
            }
        }
        else {
            throw new Error(`Unsupported organize imports mode: ${modeExpr.getText()}`);
        }
    }

    let preferences = "nil";
    if (args.length >= 3 && args[2]!.getText() !== "undefined") {
        const prefsObj = getObjectLiteralExpression(args[2]!);
        if (!prefsObj) {
            throw new Error(`Expected object literal for preferences in verify.organizeImports, got ${args[2]!.getText()}`);
        }

        const prefsFields: string[] = [];
        for (const prop of prefsObj.properties) {
            if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) {
                continue;
            }
            const propName = prop.name.text;
            const propValue = prop.initializer;

            const goFieldName = propName.charAt(0).toUpperCase() + propName.slice(1);

            if (propName === "organizeImportsIgnoreCase") {
                if (ts.isStringLiteral(propValue) && propValue.text === "auto") {
                    prefsFields.push(`${goFieldName}: core.TSUnknown`);
                }
                else if (propValue.kind === ts.SyntaxKind.TrueKeyword) {
                    prefsFields.push(`${goFieldName}: core.TSTrue`);
                }
                else if (propValue.kind === ts.SyntaxKind.FalseKeyword) {
                    prefsFields.push(`${goFieldName}: core.TSFalse`);
                }
                else {
                    throw new Error(`Unsupported value for organizeImportsIgnoreCase: ${propValue.getText()}`);
                }
            }
            else if (propName === "organizeImportsCollation") {
                if (ts.isStringLiteral(propValue)) {
                    if (propValue.text === "unicode") {
                        prefsFields.push(`${goFieldName}: lsutil.OrganizeImportsCollationUnicode`);
                    }
                    else if (propValue.text === "ordinal") {
                        prefsFields.push(`${goFieldName}: lsutil.OrganizeImportsCollationOrdinal`);
                    }
                    else {
                        throw new Error(`Unsupported value for organizeImportsCollation: ${propValue.text}`);
                    }
                }
                else {
                    throw new Error(`Expected string literal for organizeImportsCollation, got ${propValue.getText()}`);
                }
            }
            else if (propName === "organizeImportsCaseFirst") {
                if (ts.isStringLiteral(propValue)) {
                    if (propValue.text === "upper") {
                        prefsFields.push(`${goFieldName}: lsutil.OrganizeImportsCaseFirstUpper`);
                    }
                    else if (propValue.text === "lower") {
                        prefsFields.push(`${goFieldName}: lsutil.OrganizeImportsCaseFirstLower`);
                    }
                    else {
                        throw new Error(`Unsupported value for organizeImportsCaseFirst: ${propValue.text}`);
                    }
                }
                else if (propValue.kind === ts.SyntaxKind.FalseKeyword) {
                    prefsFields.push(`${goFieldName}: lsutil.OrganizeImportsCaseFirstFalse`);
                }
                else {
                    throw new Error(`Expected string literal or false for organizeImportsCaseFirst, got ${propValue.getText()}`);
                }
            }
            else if (propName === "organizeImportsTypeOrder") {
                if (ts.isStringLiteral(propValue)) {
                    const typeOrderValue = propValue.text;
                    switch (typeOrderValue) {
                        case "last":
                            prefsFields.push(`${goFieldName}: lsutil.OrganizeImportsTypeOrderLast`);
                            break;
                        case "inline":
                            prefsFields.push(`${goFieldName}: lsutil.OrganizeImportsTypeOrderInline`);
                            break;
                        case "first":
                            prefsFields.push(`${goFieldName}: lsutil.OrganizeImportsTypeOrderFirst`);
                            break;
                        default:
                            throw new Error(`Unsupported value for organizeImportsTypeOrder: ${typeOrderValue}`);
                    }
                }
                else {
                    throw new Error(`Expected string literal for organizeImportsTypeOrder, got ${propValue.getText()}`);
                }
            }
            // Boolean fields that are now Tristate
            else if (propName === "organizeImportsNumericCollation" || propName === "organizeImportsAccentCollation") {
                if (propValue.kind === ts.SyntaxKind.TrueKeyword) {
                    prefsFields.push(`${goFieldName}: core.TSTrue`);
                }
                else if (propValue.kind === ts.SyntaxKind.FalseKeyword) {
                    prefsFields.push(`${goFieldName}: core.TSFalse`);
                }
                else {
                    throw new Error(`Expected boolean for ${propName}, got ${propValue.getText()}`);
                }
            }
            // organizeImportsLocale is a plain string, not a pointer
            else if (propName === "organizeImportsLocale") {
                if (ts.isStringLiteral(propValue)) {
                    prefsFields.push(`${goFieldName}: ${rustStringLiteral(propValue.text)}.to_string()`);
                }
                else {
                    throw new Error(`Expected string literal for organizeImportsLocale, got ${propValue.getText()}`);
                }
            }
            // Default handling for other string properties
            else if (propName === "quotePreference" && ts.isStringLiteral(propValue)) {
                prefsFields.push(`${goFieldName}: ${rustQuotePreference(propValue.text)}`);
            }
            else if (propName === "importModuleSpecifierPreference" && ts.isStringLiteral(propValue)) {
                prefsFields.push(`${goFieldName}: ${rustImportModuleSpecifierPreference(propValue.text)}`);
            }
            else if (propName === "importModuleSpecifierEnding" && ts.isStringLiteral(propValue)) {
                prefsFields.push(`${goFieldName}: ${rustImportModuleSpecifierEndingPreference(propValue.text)}`);
            }
            else if (ts.isStringLiteral(propValue)) {
                prefsFields.push(`${goFieldName}: ${rustStringLiteral(propValue.text)}.to_string()`);
            }
            else if (propValue.kind === ts.SyntaxKind.TrueKeyword) {
                prefsFields.push(`${goFieldName}: core.TSTrue`);
            }
            else if (propValue.kind === ts.SyntaxKind.FalseKeyword) {
                prefsFields.push(`${goFieldName}: core.TSFalse`);
            }
            else {
                prefsFields.push(`${goFieldName}: ${propValue.getText()}`);
            }
        }

        if (prefsFields.length > 0) {
            preferences = `UserPreferences{\n${prefsFields.join(",\n")},\n..Default::default()\n}`;
        }
    }

    return [{
        kind: "verifyOrganizeImports",
        expectedContent: expectedContent.text,
        mode,
        preferences,
    }];
}

function parseBaselineSignatureHelp(args: ts.NodeArray<ts.Expression>): Cmd {
    if (args.length !== 0) {
        // All calls are currently empty!
        throw new Error("Expected no arguments in verify.baselineSignatureHelp");
    }
    return {
        kind: "verifyBaselineSignatureHelp",
    };
}

function parseSignatureHelpOptions(obj: ts.ObjectLiteralExpression): VerifySignatureHelpOptions {
    const options: VerifySignatureHelpOptions = {};

    for (const prop of obj.properties) {
        if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) {
            console.error(`Unexpected property in signatureHelp options: ${prop.getText()}`);
            continue;
        }
        const name = prop.name.text;
        const value = prop.initializer;

        switch (name) {
            case "marker": {
                if (ts.isStringLiteral(value)) {
                    options.marker = value.text;
                }
                else if (ts.isArrayLiteralExpression(value)) {
                    const markers: string[] = [];
                    for (const elem of value.elements) {
                        if (ts.isStringLiteral(elem)) {
                            markers.push(elem.text);
                        }
                        else {
                            throw new Error(`Expected string literal in marker array, got ${elem.getText()}`);
                        }
                    }
                    options.marker = markers;
                }
                else if (value.getText() === "test.markers()") {
                    options.marker = ["...test.markerNames()"];
                }
                else {
                    throw new Error(`Expected string or array for marker, got ${value.getText()}`);
                }
                break;
            }
            case "text": {
                const str = getStringLiteralLike(value);
                if (!str) {
                    throw new Error(`Expected string for text, got ${value.getText()}`);
                }
                options.text = str.text;
                break;
            }
            case "docComment": {
                const str = getStringLiteralLike(value);
                if (!str) {
                    throw new Error(`Expected string for docComment, got ${value.getText()}`);
                }
                options.docComment = str.text;
                break;
            }
            case "parameterCount": {
                const num = getNumericLiteral(value);
                if (!num) {
                    throw new Error(`Expected number for parameterCount, got ${value.getText()}`);
                }
                options.parameterCount = parseInt(num.text, 10);
                break;
            }
            case "parameterName": {
                const str = getStringLiteralLike(value);
                if (!str) {
                    throw new Error(`Expected string for parameterName, got ${value.getText()}`);
                }
                options.parameterName = str.text;
                break;
            }
            case "parameterSpan": {
                const str = getStringLiteralLike(value);
                if (!str) {
                    throw new Error(`Expected string for parameterSpan, got ${value.getText()}`);
                }
                options.parameterSpan = str.text;
                break;
            }
            case "parameterDocComment": {
                const str = getStringLiteralLike(value);
                if (!str) {
                    throw new Error(`Expected string for parameterDocComment, got ${value.getText()}`);
                }
                options.parameterDocComment = str.text;
                break;
            }
            case "overloadsCount": {
                const num = getNumericLiteral(value);
                if (!num) {
                    throw new Error(`Expected number for overloadsCount, got ${value.getText()}`);
                }
                options.overloadsCount = parseInt(num.text, 10);
                break;
            }
            case "overrideSelectedItemIndex": {
                const num = getNumericLiteral(value);
                if (!num) {
                    throw new Error(`Expected number for overrideSelectedItemIndex, got ${value.getText()}`);
                }
                options.overrideSelectedItemIndex = parseInt(num.text, 10);
                break;
            }
            case "triggerReason": {
                // triggerReason is an object like { kind: "invoked" } or { kind: "characterTyped", triggerCharacter: "(" }
                // For now, just pass it through as a string representation
                options.triggerReason = value.getText();
                break;
            }
            case "argumentCount":
                // ignore
                break;
            case "isVariadic": {
                if (value.kind === ts.SyntaxKind.TrueKeyword) {
                    options.isVariadic = true;
                }
                else if (value.kind === ts.SyntaxKind.FalseKeyword) {
                    options.isVariadic = false;
                }
                else {
                    throw new Error(`Expected boolean for isVariadic, got ${value.getText()}`);
                }
                break;
            }
            case "tags":
                // ignore
                break;
            default:
                throw new Error(`Unknown signatureHelp option: ${name}`);
        }
    }
    return options;
}

function parseSignatureHelp(args: ts.NodeArray<ts.Expression>): Cmd[] {
    const allOptions: VerifySignatureHelpOptions[] = [];

    for (const arg of args) {
        if (ts.isObjectLiteralExpression(arg)) {
            const opts = parseSignatureHelpOptions(arg);
            allOptions.push(opts);
        }
        else if (ts.isIdentifier(arg)) {
            // Could be a variable reference like `help2` - skip for now
            throw new Error(`signatureHelp with variable reference not supported: ${arg.getText()}`);
        }
        else {
            throw new Error(`Unexpected argument type in signatureHelp: ${arg.getText()}`);
        }
    }

    if (allOptions.length === 0) {
        throw new Error("signatureHelp requires at least one options object");
    }

    return [{
        kind: "verifySignatureHelp",
        options: allOptions,
    }];
}

function parseNoSignatureHelp(args: ts.NodeArray<ts.Expression>): Cmd[] {
    const markers: string[] = [];

    for (const arg of args) {
        if (ts.isStringLiteral(arg)) {
            markers.push(arg.text);
        }
        else if (ts.isSpreadElement(arg)) {
            // Handle ...test.markerNames()
            const expr = arg.expression;
            if (
                ts.isCallExpression(expr) &&
                ts.isPropertyAccessExpression(expr.expression) &&
                ts.isIdentifier(expr.expression.expression) &&
                expr.expression.expression.text === "test" &&
                ts.isIdentifier(expr.expression.name) &&
                expr.expression.name.text === "markerNames"
            ) {
                // This means "all markers" - we'll handle this specially in the generator
                return [{
                    kind: "verifyNoSignatureHelp",
                    markers: ["...test.markerNames()"],
                }];
            }
            throw new Error(`Unsupported spread in noSignatureHelp: ${arg.getText()}`);
        }
        else {
            throw new Error(`Unexpected argument in noSignatureHelp: ${arg.getText()}`);
        }
    }

    return [{
        kind: "verifyNoSignatureHelp",
        markers,
    }];
}

interface SignatureHelpTriggerReason {
    kind: "invoked" | "characterTyped" | "retrigger";
    triggerCharacter?: string;
}

function parseTriggerReason(arg: ts.Expression): SignatureHelpTriggerReason | "undefined" {
    // Handle undefined literal
    if (ts.isIdentifier(arg) && arg.text === "undefined") {
        return "undefined";
    }

    if (!ts.isObjectLiteralExpression(arg)) {
        throw new Error(`Expected object literal for trigger reason, got ${arg.getText()}`);
    }

    let kind: "invoked" | "characterTyped" | "retrigger" | undefined;
    let triggerCharacter: string | undefined;

    for (const prop of arg.properties) {
        if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) {
            throw new Error(`Unexpected property in trigger reason: ${prop.getText()}`);
        }
        const name = prop.name.text;
        if (name === "kind") {
            if (!ts.isStringLiteral(prop.initializer)) {
                throw new Error(`Expected string literal for kind, got ${prop.initializer.getText()}`);
            }
            const k = prop.initializer.text;
            if (k === "invoked" || k === "characterTyped" || k === "retrigger") {
                kind = k;
            }
            else {
                throw new Error(`Unknown trigger reason kind: ${k}`);
            }
        }
        else if (name === "triggerCharacter") {
            if (!ts.isStringLiteral(prop.initializer)) {
                throw new Error(`Expected string literal for triggerCharacter, got ${prop.initializer.getText()}`);
            }
            triggerCharacter = prop.initializer.text;
        }
    }

    if (!kind) {
        throw new Error(`Missing kind in trigger reason`);
    }

    return { kind, triggerCharacter };
}

function parseSignatureHelpPresentForTriggerReason(args: ts.NodeArray<ts.Expression>): Cmd[] {
    if (args.length === 0) {
        throw new Error("signatureHelpPresentForTriggerReason requires at least one argument");
    }

    const triggerReason = parseTriggerReason(args[0]!);

    const markers: string[] = [];
    for (let i = 1; i < args.length; i++) {
        const arg = args[i]!;
        if (ts.isStringLiteral(arg)) {
            markers.push(arg.text);
        }
        else {
            throw new Error(`Unexpected argument in signatureHelpPresentForTriggerReason: ${arg.getText()}`);
        }
    }

    return [{
        kind: "verifySignatureHelpPresent",
        triggerReason: triggerReason === "undefined" ? undefined : triggerReason,
        markers,
    }];
}

function parseNoSignatureHelpForTriggerReason(args: ts.NodeArray<ts.Expression>): Cmd[] {
    if (args.length === 0) {
        throw new Error("noSignatureHelpForTriggerReason requires at least one argument");
    }

    const triggerReason = parseTriggerReason(args[0]!);

    const markers: string[] = [];
    for (let i = 1; i < args.length; i++) {
        const arg = args[i]!;
        if (ts.isStringLiteral(arg)) {
            markers.push(arg.text);
        }
        else {
            throw new Error(`Unexpected argument in noSignatureHelpForTriggerReason: ${arg.getText()}`);
        }
    }

    return [{
        kind: "verifyNoSignatureHelpForTriggerReason",
        triggerReason: triggerReason === "undefined" ? undefined : triggerReason,
        markers,
    }];
}

function parseBaselineSmartSelection(args: ts.NodeArray<ts.Expression>): Cmd {
    if (args.length !== 0) {
        // All calls are currently empty!
        throw new Error("Expected no arguments in verify.baselineSmartSelection");
    }
    return {
        kind: "verifyBaselineSmartSelection",
    };
}

function parseBaselineCallHierarchy(args: ts.NodeArray<ts.Expression>): Cmd {
    if (args.length !== 0) {
        throw new Error("Expected no arguments in verify.baselineCallHierarchy");
    }
    return {
        kind: "verifyBaselineCallHierarchy",
    };
}

function parseOutliningSpansArgs(args: readonly ts.Expression[]): [VerifyOutliningSpansCmd] {
    if (args.length === 0) {
        throw new Error("Expected at least one argument in verify.outliningSpansInCurrentFile");
    }

    let spans: string = "";
    // Optional second argument for kind filter
    let foldingRangeKind: string | undefined;
    if (args.length > 1) {
        const kindArg = getStringLiteralLike(args[1]!);
        if (!kindArg) {
            throw new Error(`Expected string literal for outlining kind, got ${args[1]!.getText()}`);
        }
        switch (kindArg.text) {
            case "comment":
                foldingRangeKind = "lsproto.FoldingRangeKindComment";
                break;
            case "region":
                foldingRangeKind = "lsproto.FoldingRangeKindRegion";
                break;
            case "imports":
                foldingRangeKind = "lsproto.FoldingRangeKindImports";
                break;
            case "code":
                break;
            default:
                throw new Error(`Unknown folding range kind: ${kindArg.text}`);
        }
    }

    return [{
        kind: "verifyOutliningSpans",
        spans,
        foldingRangeKind,
    }];
}

function parseSemanticClassificationsAre(args: readonly ts.Expression[]): [VerifySemanticClassificationsCmd] | [] {
    if (args.length < 1) {
        throw new Error("semanticClassificationsAre requires at least a format argument");
    }

    const formatArg = args[0]!;
    if (!ts.isStringLiteralLike(formatArg)) {
        throw new Error("semanticClassificationsAre first argument must be a string literal");
    }

    const format = formatArg.text;

    // Only handle "2020" format for semantic tokens
    if (format !== "2020") {
        // Skip other formats like "original"
        return [];
    }

    const tokens: Array<{ type: string; text: string; }> = [];

    // Parse the classification tokens (c2.semanticToken("type", "text"))
    for (let i = 1; i < args.length; i++) {
        const arg = args[i]!;
        if (!ts.isCallExpression(arg)) {
            throw new Error(`Expected call expression for token at index ${i}`);
        }

        if (!ts.isPropertyAccessExpression(arg.expression) || arg.expression.name.text !== "semanticToken") {
            throw new Error(`Expected semanticToken call at index ${i}`);
        }

        if (arg.arguments.length < 2) {
            throw new Error(`semanticToken requires 2 arguments at index ${i}`);
        }

        const typeArg = arg.arguments[0]!;
        const textArg = arg.arguments[1]!;

        if (!ts.isStringLiteralLike(typeArg) || !ts.isStringLiteralLike(textArg)) {
            throw new Error(`semanticToken arguments must be string literals at index ${i}`);
        }

        // Map TypeScript's internal "member" type to LSP's "method" type
        let tokenType = typeArg.text;
        tokenType = tokenType.replace(/\bmember\b/g, "method");

        tokens.push({
            type: tokenType,
            text: textArg.text,
        });
    }

    return [{
        kind: "verifySemanticClassifications",
        format,
        tokens,
    }];
}

function parseKind(expr: ts.Expression): string {
    if (!ts.isStringLiteral(expr)) {
        throw new Error(`Expected string literal for kind, got ${expr.getText()}`);
    }
    switch (expr.text) {
        case "primitive type":
        case "keyword":
            return "lsproto.CompletionItemKindKeyword";
        case "const":
        case "let":
        case "var":
        case "local var":
        case "alias":
        case "parameter":
            return "lsproto.CompletionItemKindVariable";
        case "property":
        case "getter":
        case "setter":
            return "lsproto.CompletionItemKindField";
        case "function":
        case "local function":
            return "lsproto.CompletionItemKindFunction";
        case "method":
        case "construct":
        case "call":
        case "index":
            return "lsproto.CompletionItemKindMethod";
        case "enum":
            return "lsproto.CompletionItemKindEnum";
        case "enum member":
            return "lsproto.CompletionItemKindEnumMember";
        case "module":
        case "external module name":
            return "lsproto.CompletionItemKindModule";
        case "class":
        case "type":
            return "lsproto.CompletionItemKindClass";
        case "interface":
            return "lsproto.CompletionItemKindInterface";
        case "warning":
            return "lsproto.CompletionItemKindText";
        case "script":
            return "lsproto.CompletionItemKindFile";
        case "directory":
            return "lsproto.CompletionItemKindFolder";
        case "string":
            return "lsproto.CompletionItemKindConstant";
        default:
            return "lsproto.CompletionItemKindProperty";
    }
}

const fileKindModifiers = new Set([".d.ts", ".ts", ".tsx", ".js", ".jsx", ".json"]);

function parseKindModifiers(expr: ts.Expression): { isOptional: boolean; isDeprecated: boolean; extensions: string[]; } {
    if (!ts.isStringLiteral(expr)) {
        throw new Error(`Expected string literal for kind modifiers, got ${expr.getText()}`);
    }
    let isOptional = false;
    let isDeprecated = false;
    const extensions: string[] = [];
    const modifiers = expr.text.split(",");
    for (const modifier of modifiers) {
        switch (modifier) {
            case "optional":
                isOptional = true;
                break;
            case "deprecated":
                isDeprecated = true;
                break;
            default:
                if (fileKindModifiers.has(modifier)) {
                    extensions.push(modifier);
                }
        }
    }
    return {
        isOptional,
        isDeprecated,
        extensions,
    };
}

interface ParsedSortText {
    expression: string;
    deprecated: boolean;
}

function parseSortText(expr: ts.Expression): ParsedSortText {
    if (ts.isCallExpression(expr) && expr.expression.getText() === "completion.SortText.Deprecated") {
        const inner = parseSortText(expr.arguments[0]!);
        return {
            expression: `ls.DeprecateSortText(${inner.expression})`,
            deprecated: true,
        };
    }

    return { expression: parseSortTextExpression(expr.getText()), deprecated: false };
}

function parseSortTextExpression(text: string): string {
    switch (text) {
        case "completion.SortText.LocalDeclarationPriority":
            return "ls.SortTextLocalDeclarationPriority";
        case "completion.SortText.LocationPriority":
            return "ls.SortTextLocationPriority";
        case "completion.SortText.OptionalMember":
            return "ls.SortTextOptionalMember";
        case "completion.SortText.MemberDeclaredBySpreadAssignment":
            return "ls.SortTextMemberDeclaredBySpreadAssignment";
        case "completion.SortText.SuggestedClassMembers":
            return "ls.SortTextSuggestedClassMembers";
        case "completion.SortText.GlobalsOrKeywords":
            return "ls.SortTextGlobalsOrKeywords";
        case "completion.SortText.AutoImportSuggestions":
            return "ls.SortTextAutoImportSuggestions";
        case "completion.SortText.ClassMemberSnippets":
            return "ls.SortTextClassMemberSnippets";
        case "completion.SortText.JavascriptIdentifiers":
            return "ls.SortTextJavascriptIdentifiers";
        default:
            throw new Error(`Unrecognized sort text: ${text}`); // !!! support deprecated/obj literal prop/etc
    }
}

function formatCompletionItemTags(tags: Set<string>): string | undefined {
    if (tags.size === 0) {
        return undefined;
    }
    return `Tags: &[]lsproto.CompletionItemTag{${[...tags].join(", ")}},`;
}

function parseVerifyNavigateTo(args: ts.NodeArray<ts.Expression>, env: ParseEnv = {}): [VerifyNavToCmd | VerifyNavToEachRangeCmd] {
    if (env.rangesVar && args.length === 1) {
        const rangeLoopPattern = getRangeLoopNavigateToPattern(args[0]!, env);
        if (rangeLoopPattern) {
            return [{ kind: "verifyNavigateToEachRange", pattern: rangeLoopPattern }];
        }
    }
    const navArgs = [];
    for (const arg of args) {
        const result = parseVerifyNavigateToArg(arg);
        navArgs.push(result);
    }
    return [{
        kind: "verifyNavigateTo",
        args: navArgs,
    }];
}

function getRangeLoopNavigateToPattern(arg: ts.Expression | undefined, env: ParseEnv): RangeLoopNavigateToPattern | undefined {
    if (!arg || !ts.isObjectLiteralExpression(arg) || !env.rangesVar) return undefined;
    const rangeVar = env.rangesVar;
    let pattern: RangeLoopNavigateToPattern | undefined;
    let hasExpected = false;
    for (const prop of arg.properties) {
        if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) continue;
        const initializerText = prop.initializer.getText();
        const compactInitializerText = initializerText.replace(/\s+/g, "");
        if (prop.name.text === "pattern" && initializerText === `${rangeVar}.marker.data.name`) {
            pattern = "name";
        }
        else if (prop.name.text === "pattern" && initializerText === `${rangeVar}.marker.data.name.slice(2)`) {
            pattern = "substringFrom2";
        }
        else if (
            prop.name.text === "pattern"
            && env.rangeDataNameAlias
            && compactInitializerText === `${env.rangeDataNameAlias}.slice(0,${env.rangeDataNameAlias}.length-1)`
        ) {
            pattern = "prefixDropLast";
        }
        if (prop.name.text === "expected" && ts.isArrayLiteralExpression(prop.initializer) && prop.initializer.elements.length === 1) {
            const element = prop.initializer.elements[0]!;
            hasExpected = !!element && ts.isObjectLiteralExpression(element) && element.properties.some(
                itemProp => ts.isSpreadAssignment(itemProp) && itemProp.expression.getText() === `${rangeVar}.marker.data`,
            ) && element.properties.some(
                itemProp => ts.isShorthandPropertyAssignment(itemProp) && itemProp.name.text === rangeVar,
            );
        }
    }
    return pattern && hasExpected ? pattern : undefined;
}

function parseVerifyNavigateToArg(arg: ts.Expression): VerifyNavToArg {
    if (!ts.isObjectLiteralExpression(arg)) {
        throw new Error(`Expected object literal expression for verify.navigateTo argument, got ${arg.getText()}`);
    }
    let excludeLibFiles: boolean | undefined;
    const items: NavToItem[] = [];
    let pattern: string | undefined;
    for (const prop of arg.properties) {
        if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) {
            throw new Error(`Expected property assignment with identifier name for verify.navigateTo argument, got ${prop.getText()}`);
        }
        const propName = prop.name.text;
        switch (propName) {
            case "pattern": {
                let patternInit = getStringLiteralLike(prop.initializer);
                if (!patternInit) {
                    throw new Error(`Expected string literal for pattern in verify.navigateTo argument, got ${prop.initializer.getText()}`);
                }
                pattern = patternInit.text;
                break;
            }
            case "fileName":
                // no longer supported
                continue;
            case "expected": {
                const init = prop.initializer;
                if (!ts.isArrayLiteralExpression(init)) {
                    throw new Error(`Expected array literal expression for expected property in verify.navigateTo argument, got ${init.getText()}`);
                }
                for (const elem of init.elements) {
                    const result = parseNavToItem(elem);
                    items.push(result);
                }
                break;
            }
            case "excludeLibFiles": {
                if (prop.initializer.kind === ts.SyntaxKind.TrueKeyword) {
                    excludeLibFiles = true;
                }
                else if (prop.initializer.kind === ts.SyntaxKind.FalseKeyword) {
                    excludeLibFiles = false;
                }
                else {
                    throw new Error(`Expected boolean literal for excludeLibFiles, got ${prop.initializer.getText()}`);
                }
            }
        }
    }
    return {
        pattern: pattern ?? "",
        preferences: excludeLibFiles === undefined ? undefined : { excludeLibFiles },
        exact: items,
    };
}

function parseVerifyNavTree(args: readonly ts.Expression[]): [VerifyNavTreeCmd] {
    // Ignore arguments and use baseline tests intead.
    return [{
        kind: "verifyNavigationTree",
    }];
}

function parseNavToItem(arg: ts.Expression): NavToItem {
    let item = getNodeOfKind(arg, ts.isObjectLiteralExpression);
    if (!item) {
        throw new Error(`Expected object literal expression for navigateTo item, got ${arg.getText()}`);
    }
    let name: string | undefined;
    let kind: string | undefined;
    let tags: "deprecated" | undefined;
    let location: string | undefined;
    let containerName: string | undefined;
    for (const prop of item.properties) {
        if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) {
            throw new Error(`Expected property assignment with identifier name for navigateTo item, got ${prop.getText()}`);
        }
        const propName = prop.name.text;
        const init = prop.initializer;
        switch (propName) {
            case "name": {
                let nameInit;
                if (!(nameInit = getStringLiteralLike(init))) {
                    throw new Error(`Expected string literal for name in navigateTo item, got ${init.getText()}`);
                }
                name = nameInit.text;
                break;
            }
            case "kind": {
                kind = getSymbolKind(init);
                break;
            }
            case "kindModifiers": {
                if (init.getText().includes("deprecated")) {
                    tags = "deprecated";
                }
                break;
            }
            case "range": {
                if (ts.isIdentifier(init) || (ts.isElementAccessExpression(init) && ts.isIdentifier(init.expression))) {
                    let parsedRange = parseRangeVariable(init);
                    if (parsedRange) {
                        location = parsedRange;
                        continue;
                    }
                }
                if (ts.isElementAccessExpression(init) && init.expression.getText() === "test.ranges()") {
                    location = `f.Ranges()[${parseInt(init.argumentExpression.getText())}]`;
                    continue;
                }
                throw new Error(`Expected range variable for range in navigateTo item, got ${init.getText()}`);
            }
            case "containerName": {
                let nameInit;
                if (!(nameInit = getStringLiteralLike(init))) {
                    throw new Error(`Expected string literal for container name in navigateTo item, got ${init.getText()}`);
                }
                containerName = nameInit.text;
                break;
            }
            default:
                // ignore other properties
        }
    }
    if (!name || !kind || !location) {
        throw new Error(`Missing required navigateTo item property in ${arg.getText()}`);
    }
    return { name, kind, tags, location, containerName };
}

function getSymbolKind(kind: ts.Expression): string {
    let result;
    if (!(result = getStringLiteralLike(kind))) {
        throw new Error(`Expected string literal for symbol kind, got ${kind.getText()}`);
    }
    return getSymbolKindWorker(result.text);
}

function getSymbolKindWorker(kind: string): string {
    switch (kind) {
        case "script":
            return "SymbolKindFile";
        case "module":
            return "SymbolKindNamespace";
        case "class":
        case "local class":
            return "SymbolKindClass";
        case "interface":
            return "SymbolKindInterface";
        case "type":
            return "SymbolKindClass";
        case "enum":
            return "SymbolKindEnum";
        case "enum member":
            return "SymbolKindEnumMember";
        case "var":
        case "local var":
        case "using":
        case "await using":
            return "SymbolKindVariable";
        case "function":
        case "local function":
            return "SymbolKindFunction";
        case "method":
            return "SymbolKindMethod";
        case "getter":
        case "setter":
        case "property":
        case "accessor":
            return "SymbolKindProperty";
        case "constructor":
        case "construct":
            return "SymbolKindConstructor";
        case "call":
        case "index":
            return "SymbolKindFunction";
        case "parameter":
            return "SymbolKindVariable";
        case "type parameter":
            return "SymbolKindTypeParameter";
        case "primitive type":
            return "SymbolKindObject";
        case "const":
        case "let":
            return "SymbolKindVariable";
        case "directory":
            return "SymbolKindPackage";
        case "external module name":
            return "SymbolKindModule";
        case "string":
            return "SymbolKindString";
        case "type":
            return "SymbolKindClass";
        default:
            return "SymbolKindVariable";
    }
}

interface VerifyCompletionsCmd {
    kind: "verifyCompletions";
    marker: CompletionMarkerInput;
    isNewIdentifierLocation?: true;
    args?: VerifyCompletionsArgs | "nil";
    andApplyCodeActionArgs?: VerifyApplyCodeActionArgs;
}

type CompletionMarkerInput =
    | { kind: "none" }
    | { kind: "name"; name: string }
    | { kind: "names"; names: string[] }
    | { kind: "marker"; name: string }
    | { kind: "allMarkers" };

interface VerifyCompletionsArgs {
    includes?: string;
    excludes?: string[];
    exact?: string;
    unsorted?: string;
    preferences: string;
}

interface VerifyApplyCodeActionArgs {
    name: string;
    source: string;
    description: string;
    newFileContent: string;
}

interface ApplyCodeActionFromCompletionOptions {
    name: string;
    source: string;
    description: string;
    autoImportFix: boolean;
    newFileContent?: string;
    newRangeContent?: string;
}

interface VerifyApplyCodeActionFromCompletionCmd {
    kind: "verifyApplyCodeActionFromCompletion";
    marker?: string;
    options: ApplyCodeActionFromCompletionOptions;
}

type BaselineMarkerArg =
    | { kind: "name"; name: string }
    | { kind: "allMarkerNames" };

interface VerifyBaselineFindAllReferencesCmd {
    kind: "verifyBaselineFindAllReferences";
    markers: BaselineMarkerArg[];
    ranges?: boolean;
}

interface VerifyBaselineGoToDefinitionCmd {
    kind: "verifyBaselineGoToDefinition" | "verifyBaselineGoToType" | "verifyBaselineGoToImplementation" | "verifyBaselineGoToSourceDefinition";
    markers: BaselineMarkerArg[];
    boundSpan?: true;
    ranges?: boolean;
}

interface VerifyBaselineQuickInfoCmd {
    kind: "verifyBaselineQuickInfo";
    verbosityLevels?: Record<string, number[]>;
}

interface VerifyBaselineSignatureHelpCmd {
    kind: "verifyBaselineSignatureHelp";
}

interface VerifyBaselineSmartSelection {
    kind: "verifyBaselineSmartSelection";
}

interface VerifyBaselineCallHierarchy {
    kind: "verifyBaselineCallHierarchy";
}

interface VerifyBaselineRenameCmd {
    kind: "verifyBaselineRename" | "verifyBaselineRenameAtRangesWithText";
    args: string[];
    preferences: string;
}

interface VerifyBaselineDocumentHighlightsCmd {
    kind: "verifyBaselineDocumentHighlights";
    args: string[];
    preferences: string;
    filesToSearch?: string[];
}

interface VerifyBaselineCompletionsCmd {
    kind: "verifyBaselineCompletions" | "verifyBaselineAutoImports";
}

interface VerifyBaselineInlayHintsCmd {
    kind: "verifyBaselineInlayHints";
    span: string;
    preferences: string;
}

interface VerifyImportFixAtPositionCmd {
    kind: "verifyImportFixAtPosition";
    expectedTexts: string[];
    preferences: string;
}

interface VerifyImportFixModuleSpecifiersCmd {
    kind: "verifyImportFixModuleSpecifiers";
    markerName: string;
    moduleSpecifiers: string[];
    preferences: string;
}

interface GoToCmd {
    kind: "goTo";
    // !!! `selectRange` and `rangeStart` require parsing variables and `test.ranges()[n]`
    funcName: "marker" | "file" | "fileNumber" | "EOF" | "BOF" | "position" | "select";
    marker?: string;
    file?: string;
    fileNumber?: number;
    position?: number;
    startMarker?: string;
    endMarker?: string;
}

interface EditCmd {
    kind: "edit";
    action: "disableFormatting" | "insert" | "paste" | "insertLine" | "replaceLine" | "backspace" | "deleteAtCaret" | "deleteLine";
    text?: string;
    line?: number;
    count?: number;
}

interface FormatCmd {
    kind: "format";
    action: "document" | "setOption" | "selection";
    option?: string;
    value?: string;
    startMarker?: string;
    endMarker?: string;
}

interface VerifyContentCmd {
    kind: "verifyContent";
    assertion: "currentFileContentIs" | "currentLineContentIs" | "indentationIs";
    text: string;
}

interface VerifyIndentationAtMarkersFromDataCmd {
    kind: "verifyIndentationAtMarkersFromData";
}

interface VerifyQuickInfoCmd {
    kind: "quickInfoIs" | "quickInfoAt" | "quickInfoAtEachMarker" | "quickInfoExists" | "notQuickInfoExists";
    marker?: string;
    text?: string;
    docs?: string;
}

interface VerifyOrganizeImportsCmd {
    kind: "verifyOrganizeImports";
    expectedContent: string;
    mode: string;
    preferences: string;
}

interface VerifyRenameInfoCmd {
    kind: "renameInfoSucceeded" | "renameInfoFailed";
    preferences: string;
}

interface VerifyGetEditsForFileRenameCmd {
    kind: "verifyGetEditsForFileRename";
    oldPath: string;
    newPath: string;
    newFileContents: RenameFileContent[];
    preferences: string;
}

interface RenameFileContent {
    path: string;
    content: string;
}

interface VerifyBaselineLinkedEditingCmd {
    kind: "verifyBaselineLinkedEditing";
}
interface VerifyLinkedEditingCmd {
    kind: "verifyLinkedEditing";
    ranges: string;
}

interface VerifyDiagnosticsCmd {
    kind: "verifyDiagnostics";
    arg: string;
    isSuggestion: boolean;
}

interface VerifyBaselineDiagnosticsCmd {
    kind: "verifyBaselineDiagnostics";
}

interface VerifyNavToCmd {
    kind: "verifyNavigateTo";
    args: VerifyNavToArg[];
}

interface VerifyNavToEachRangeCmd {
    kind: "verifyNavigateToEachRange";
    pattern: RangeLoopNavigateToPattern;
}

type RangeLoopNavigateToPattern = "name" | "prefixDropLast" | "substringFrom2";

interface VerifyNavToArg {
    pattern: string;
    preferences?: VerifyNavToPreferences;
    exact: NavToItem[];
}

interface VerifyNavToPreferences {
    excludeLibFiles: boolean;
}

interface NavToItem {
    name: string;
    kind: string;
    tags?: "deprecated";
    location: string;
    containerName?: string;
}

interface VerifySignatureHelpOptions {
    marker?: string | string[];
    text?: string;
    docComment?: string;
    parameterCount?: number;
    parameterName?: string;
    parameterSpan?: string;
    parameterDocComment?: string;
    overloadsCount?: number;
    overrideSelectedItemIndex?: number;
    triggerReason?: string;
    isVariadic?: boolean;
}

interface VerifySignatureHelpCmd {
    kind: "verifySignatureHelp";
    options: VerifySignatureHelpOptions[];
}

interface VerifyNoSignatureHelpCmd {
    kind: "verifyNoSignatureHelp";
    markers: string[];
}

interface VerifySignatureHelpPresentCmd {
    kind: "verifySignatureHelpPresent";
    triggerReason?: SignatureHelpTriggerReason;
    markers: string[];
}

interface VerifyNoSignatureHelpForTriggerReasonCmd {
    kind: "verifyNoSignatureHelpForTriggerReason";
    triggerReason?: SignatureHelpTriggerReason;
    markers: string[];
}

interface VerifyOutliningSpansCmd {
    kind: "verifyOutliningSpans";
    spans: string;
    foldingRangeKind?: string;
}

interface VerifyNavTreeCmd {
    kind: "verifyNavigationTree";
}

interface VerifyNumberOfErrorsInCurrentFileCmd {
    kind: "verifyNumberOfErrorsInCurrentFile";
    expectedCount: number;
}

interface VerifyNoErrorsCmd {
    kind: "verifyNoErrors";
}

interface VerifyErrorExistsAtRangeCmd {
    kind: "verifyErrorExistsAtRange";
    range: string;
    code: number;
    message: string;
}

interface VerifyCurrentLineContentIsCmd {
    kind: "verifyCurrentLineContentIs";
    text: string;
}

interface VerifyCurrentFileContentIsCmd {
    kind: "verifyCurrentFileContentIs";
    text: string;
}

interface VerifyErrorExistsBetweenMarkersCmd {
    kind: "verifyErrorExistsBetweenMarkers" | "verifyNoErrorExistsBetweenMarkers";
    startMarker: string;
    endMarker: string;
}

interface VerifyErrorExistsAfterMarkerCmd {
    kind: "verifyErrorExistsAfterMarker" | "verifyNoErrorExistsAfterMarker";
    markerName: string;
}

interface VerifyErrorExistsBeforeMarkerCmd {
    kind: "verifyErrorExistsBeforeMarker" | "verifyNoErrorExistsBeforeMarker";
    markerName: string;
}

interface VerifyCodeFixCmd {
    kind: "verifyCodeFix";
    description: string;
    newFileContent?: string;
    newRangeContent?: string;
    index: number;
    applyChanges: boolean;
    preferences: string;
}

interface VerifyCodeFixAvailableCmd {
    kind: "verifyCodeFixAvailable";
    descriptions: string[];
    unavailableDescriptions: string[];
    expectNone: boolean;
}

interface VerifyRangeAfterCodeFixCmd {
    kind: "verifyRangeAfterCodeFix";
    expectedText: string;
    includeWhiteSpace: boolean;
    errorCode: number;
    index: number;
}

interface VerifyCodeFixAllCmd {
    kind: "verifyCodeFixAll";
    fixId: string;
    newFileContent: string;
}

interface VerifyCodeFixAllAvailableCmd {
    kind: "verifyCodeFixAllNotAvailable";
    fixId: string;
}

interface VerifySemanticClassificationsCmd {
    kind: "verifySemanticClassifications";
    format: string;
    tokens: Array<{ type: string; text: string; }>;
}

type CmdData =
    | VerifyCompletionsCmd
    | VerifyApplyCodeActionFromCompletionCmd
    | VerifyBaselineFindAllReferencesCmd
    | VerifyBaselineDocumentHighlightsCmd
    | VerifyBaselineCompletionsCmd
    | VerifyBaselineGoToDefinitionCmd
    | VerifyBaselineQuickInfoCmd
    | VerifyBaselineSignatureHelpCmd
    | VerifyBaselineSmartSelection
    | VerifySignatureHelpCmd
    | VerifyNoSignatureHelpCmd
    | VerifySignatureHelpPresentCmd
    | VerifyNoSignatureHelpForTriggerReasonCmd
    | VerifyBaselineCallHierarchy
    | GoToCmd
    | FormatCmd
    | EditCmd
    | VerifyContentCmd
    | VerifyIndentationAtMarkersFromDataCmd
    | VerifyQuickInfoCmd
    | VerifyOrganizeImportsCmd
    | VerifyBaselineRenameCmd
    | VerifyRenameInfoCmd
    | VerifyGetEditsForFileRenameCmd
    | VerifyBaselineLinkedEditingCmd
    | VerifyLinkedEditingCmd
    | VerifyNavToCmd
    | VerifyNavToEachRangeCmd
    | VerifyNavTreeCmd
    | VerifyBaselineInlayHintsCmd
    | VerifyImportFixAtPositionCmd
    | VerifyImportFixModuleSpecifiersCmd
    | VerifyDiagnosticsCmd
    | VerifyBaselineDiagnosticsCmd
    | VerifySemanticClassificationsCmd
    | VerifyOutliningSpansCmd
    | VerifyNumberOfErrorsInCurrentFileCmd
    | VerifyNoErrorsCmd
    | VerifyErrorExistsAtRangeCmd
    | VerifyCurrentLineContentIsCmd
    | VerifyCurrentFileContentIsCmd
    | VerifyErrorExistsBetweenMarkersCmd
    | VerifyErrorExistsAfterMarkerCmd
    | VerifyErrorExistsBeforeMarkerCmd
    | VerifyCodeFixCmd
    | VerifyCodeFixAvailableCmd
    | VerifyRangeAfterCodeFixCmd
    | VerifyCodeFixAllCmd
    | VerifyCodeFixAllAvailableCmd;

type Cmd = CmdData & {
    comments?: string[];
};

interface GoTest {
    name: string;
    content: string;
    commands: Cmd[];
}

function generateRustTest(test: GoTest, isServer: boolean): string {
    const testName = "test_" + toSnakeCase(test.name);
    const upstreamTestName = "Test" + capitalizeIdentifier(test.name);
    const content = rustRawStringFromGoRaw(test.content);
    const commands = test.commands.map(cmd => generateRustCmd(cmd)).join("\n");
    validateGeneratedRustCommands(test.name, commands);
    const fBinding = isServer || test.commands.some(usesMutableFourslashHarness) ? "mut f" : "f";
    const template = `#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn ${testName}() {
    let mut t = TestingT;
    run_${testName}(&mut t);
}

fn run_${testName}(t: &mut TestingT) {
    if should_skip_if_failing(${rustStringLiteral(upstreamTestName)}) {
        return;
    }
    let content = ${content};
    let (${fBinding}, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    ${isServer ? `f.mark_test_as_strada_server();\n    ` : ""}${commands}
    done();
}`;
    return template;
}

function usesMutableFourslashHarness(cmd: Cmd): boolean {
    switch (cmd.kind) {
        case "verifyNavigateTo":
        case "verifyNavigateToEachRange":
            return false;
        default:
            return true;
    }
}

function validateGeneratedRustCommands(testName: string, commands: string): void {
    const unsupportedPatterns: Array<[RegExp, string]> = [
        [/\bCompletionItemData\s*\{/, "completion item data literals need a typed helper"],
        [/\bAutoImportFix\s*\{/, "auto-import completion data literals need a typed helper"],
        [/\bTextEditOrInsertReplaceEdit\b/, "completion edit range data is not emitted cleanly yet"],
        [/\bInsertReplaceEdit\b/, "insert/replace edit data is not emitted cleanly yet"],
        [/fourslash\.AnyTextEdits\b/, "additional text edits sentinel needs a typed helper"],
        [/\bStringOrMarkupContent\s*\{/, "markup content literals need a typed helper"],
        [/\bMarkupContent\s*\{/, "markup content literals need a typed helper"],
        [/\blsproto::Diagnostic\s*\{/, "LSP diagnostic literals need a typed helper"],
        [/\bstring\(ls::/, "ls sort text constants need direct Rust mappings"],
    ];

    for (const [pattern, reason] of unsupportedPatterns) {
        if (pattern.test(commands)) {
            throw new Error(`${testName}: unsupported generated Rust shape: ${reason}`);
        }
    }
}

function generateRustCmd(cmd: Cmd): string {
    const statement = generateRustCmdWorker(cmd);
    if (!cmd.comments || cmd.comments.length === 0) {
        return statement;
    }
    return `${statement}\n${cmd.comments.flatMap(rustCommentLines).join("\n")}`;
}

function rustCommentLines(comment: string): string[] {
    const trimmed = comment.trim();
    if (trimmed.startsWith("//")) {
        return comment.split(/\r?\n/).map(line => line.trimEnd());
    }

    let content = trimmed;
    if (content.startsWith("/*")) {
        content = content.slice(2);
    }
    if (content.endsWith("*/")) {
        content = content.slice(0, -2);
    }

    return content
        .split(/\r?\n/)
        .map(line => line.replace(/^\s*\*\s?/, "").trimEnd())
        .map(line => line.length === 0 ? "//" : `// ${line}`);
}

function generateRustCmdWorker(cmd: Cmd): string {
    switch (cmd.kind) {
        case "goTo":
            return generateRustGoToCommand(cmd);
        case "edit":
            return generateRustEditCommand(cmd);
        case "format":
            return generateRustFormatCommand(cmd);
        case "verifyContent":
            return generateRustVerifyContentCommand(cmd);
        case "verifyIndentationAtMarkersFromData":
            return "f.verify_indentation_at_markers_from_data(t);";
        case "quickInfoAt":
        case "quickInfoAtEachMarker":
        case "quickInfoIs":
        case "quickInfoExists":
        case "notQuickInfoExists":
            return generateRustQuickInfoCommand(cmd);
        case "verifyCompletions":
            return generateRustVerifyCompletions(cmd);
        case "verifyBaselineFindAllReferences":
            return `f.verify_baseline_find_all_references(t, ${rustBaselineMarkerStringSlice(cmd.markers, cmd.ranges)});`;
        case "verifyBaselineGoToDefinition":
            return `f.verify_baseline_go_to_definition(t, ${rustBaselineMarkerStringSlice(cmd.markers, cmd.ranges)});`;
        case "verifyBaselineGoToType":
            return `f.verify_baseline_go_to_type_definition(t, ${rustBaselineMarkerStringSlice(cmd.markers, cmd.ranges)});`;
        case "verifyBaselineGoToImplementation":
            return `f.verify_baseline_go_to_implementation(t, ${rustBaselineMarkerStringSlice(cmd.markers, cmd.ranges)});`;
        case "verifyBaselineGoToSourceDefinition":
            return `f.verify_baseline_go_to_source_definition(t, ${rustBaselineMarkerStringSlice(cmd.markers, cmd.ranges)});`;
        case "verifyBaselineQuickInfo":
            return generateRustBaselineQuickInfo(cmd);
        case "verifyBaselineSignatureHelp":
            return "f.verify_baseline_signature_help(t, &[]);";
        case "verifyBaselineSmartSelection":
            return "f.verify_baseline_selection_ranges(t, &[]);";
        case "verifyBaselineCallHierarchy":
            return "f.verify_baseline_call_hierarchy(t);";
        case "verifyBaselineDocumentHighlights":
            return generateRustBaselineDocumentHighlights(cmd);
        case "verifyBaselineCompletions":
            return "f.verify_baseline_completions(t, &[]);";
        case "verifyBaselineAutoImports":
            return "f.baseline_auto_imports_completions(t, &[]);";
        case "verifyBaselineRename":
            return generateRustBaselineRename(cmd);
        case "verifyBaselineRenameAtRangesWithText":
            return `f.verify_baseline_rename_at_ranges_with_text(t, ${goExprToRust(cmd.args[0]! ?? "\"\"")});`;
        case "verifyBaselineInlayHints":
            return `f.verify_baseline_inlay_hints(t);`;
        case "verifyBaselineDiagnostics":
            return "f.verify_baseline_non_suggestion_diagnostics(t);";
        case "verifyNavigationTree":
            return "f.verify_baseline_document_symbol(t);";
        case "verifyOutliningSpans":
            return "f.verify_outlining_spans_from_ranges(t);";
        case "verifySignatureHelp":
            return generateRustSignatureHelp(cmd);
        case "verifyNoSignatureHelp":
            return generateRustNoSignatureHelp(cmd);
        case "verifySignatureHelpPresent":
            return generateRustSignatureHelpPresent(cmd);
        case "verifyNoSignatureHelpForTriggerReason":
            return generateRustNoSignatureHelpForTriggerReason(cmd);
        case "verifyImportFixAtPosition":
            return generateRustImportFixAtPosition(cmd);
        case "verifyImportFixModuleSpecifiers":
            return generateRustImportFixModuleSpecifiers(cmd);
        case "verifyCodeFixAvailable":
            return generateRustCodeFixAvailable(cmd);
        case "verifyRangeAfterCodeFix":
            return `f.verify_range_after_code_fix(t, ${rustStringLiteral(cmd.expectedText)}, ${cmd.includeWhiteSpace}, ${cmd.errorCode}, ${cmd.index});`;
        case "verifyOrganizeImports":
            return `f.verify_organize_imports(t, ${rustRawString(cmd.expectedContent)}, ${rustCodeActionKind(cmd.mode)}, ${rustOptionalGoExpr(cmd.preferences)});`;
        case "verifyApplyCodeActionFromCompletion":
            return generateRustApplyCodeActionFromCompletion(cmd);
        case "verifySemanticClassifications":
            return generateRustSemanticClassifications(cmd);
        case "renameInfoSucceeded":
            return "f.verify_rename_succeeded_at_current_position();";
        case "renameInfoFailed":
            return "f.verify_rename_failed_at_current_position();";
        case "verifyBaselineLinkedEditing":
            return "f.verify_baseline_linked_editing(t);";
        case "verifyErrorExistsBeforeMarker":
            return `f.verify_error_exists_before_marker(&f.marker_by_name(${rustStringLiteral(cmd.markerName)}), 0);`;
        case "verifyNoErrorExistsBeforeMarker":
            return `f.verify_no_error_exists_before_marker_name(${rustStringLiteral(cmd.markerName)});`;
        case "verifyCodeFixAll":
            return `f.verify_code_fix_all(t, VerifyCodeFixAllOptions {
    fix_id: ${rustStringLiteral(cmd.fixId)}.to_string(),
    new_file_content: ${rustRawString(cmd.newFileContent)}.to_string(),
});`;
        case "verifyCodeFixAllNotAvailable":
            return `f.verify_code_fix_all_not_available(t, ${rustStringLiteral(cmd.fixId)});`;
        case "verifyCodeFix":
            return `f.verify_code_fix(t, VerifyCodeFixOptions {
    description: ${rustStringLiteral(cmd.description)}.to_string(),
    new_file_content: ${cmd.newFileContent === undefined ? "String::new()" : `${rustRawString(cmd.newFileContent)}.to_string()`},
    new_range_content: ${cmd.newRangeContent === undefined ? "String::new()" : `${rustRawString(cmd.newRangeContent)}.to_string()`},
    index: ${cmd.index},
    apply_changes: ${cmd.applyChanges},
    user_preferences: ${rustOptionalGoExpr(cmd.preferences)},
});`;
        case "verifyNoErrors":
            return "f.verify_no_errors();";
        case "verifyDiagnostics":
            return `f.${cmd.isSuggestion ? "verify_suggestion_diagnostics" : "verify_non_suggestion_diagnostics"}(${rustDiagnosticsArg(cmd.arg)});`;
        case "verifyNumberOfErrorsInCurrentFile":
            return `f.verify_number_of_errors_in_current_file(${cmd.expectedCount});`;
        case "verifyCurrentLineContentIs":
            return `f.verify_current_line_content(t, ${rustStringLiteral(cmd.text)});`;
        case "verifyCurrentFileContentIs":
            return `f.verify_current_file_content(t, ${rustRawString(cmd.text)});`;
        case "verifyErrorExistsAfterMarker":
            return `f.verify_error_exists_after_marker_name(${rustStringLiteral(cmd.markerName)});`;
        case "verifyNoErrorExistsAfterMarker":
            return `f.verify_no_error_exists_after_marker_name(${rustStringLiteral(cmd.markerName)});`;
        case "verifyErrorExistsBetweenMarkers":
            return `f.verify_error_exists_between_markers(&f.marker_by_name(${rustStringLiteral(cmd.startMarker)}), &f.marker_by_name(${rustStringLiteral(cmd.endMarker)}));`;
        case "verifyNoErrorExistsBetweenMarkers":
            return `f.verify_no_error_exists_between_markers(&f.marker_by_name(${rustStringLiteral(cmd.startMarker)}), &f.marker_by_name(${rustStringLiteral(cmd.endMarker)}));`;
        case "verifyGetEditsForFileRename":
            return `f.verify_will_rename_files_edits(t, ${rustStringLiteral(cmd.oldPath)}, ${rustStringLiteral(cmd.newPath)}, ${rustRenameFileContents(cmd.newFileContents)});`;
        case "verifyNavigateTo":
            return generateRustNavigateTo(cmd);
        case "verifyNavigateToEachRange":
            return generateRustNavigateToEachRange(cmd);
        default:
            throw new Error(`No Rust emitter for fourslash command kind: ${cmd.kind}`);
    }
}

function rustBaselineMarkerStringSlice(markers: BaselineMarkerArg[], useRanges?: boolean): string {
    if (useRanges || markers.length === 0) return "&[]";
    if (markers.some(marker => marker.kind === "allMarkerNames")) return "&f.marker_names()";
    return `&[${markers.map(marker => {
        if (marker.kind !== "name") throw new Error(`Unexpected marker spread in named marker list`);
        return `${rustStringLiteral(marker.name)}.to_string()`;
    }).join(", ")}]`;
}

function rustMarkerStringSlice(markers: string[], useRanges?: boolean): string {
    if (useRanges || markers.length === 0) return "&[]";
    if (markers.some(marker => marker.includes("MarkerNames"))) return "&f.marker_names()";
    return `&[${markers.map(marker => `${goExprToRust(marker)}.to_string()`).join(", ")}]`;
}

function generateRustBaselineQuickInfo(cmd: VerifyBaselineQuickInfoCmd): string {
    if (!cmd.verbosityLevels || Object.keys(cmd.verbosityLevels).length === 0) {
        return "f.verify_baseline_hover(t, &[]);";
    }
    const entries = Object.entries(cmd.verbosityLevels).map(
        ([marker, levels]) => `(${rustStringLiteral(marker)}.to_string(), vec![${levels.join(", ")}])`,
    );
    return `f.verify_baseline_hover_with_verbosity_by_marker(t, std::collections::BTreeMap::from([${entries.join(", ")}]));`;
}

function generateRustBaselineDocumentHighlights(cmd: VerifyBaselineDocumentHighlightsCmd): string {
    const preferences = rustOptionalGoExpr(cmd.preferences);
    const args = rustMarkerOrRangeOrNameVec(cmd.args);
    if (cmd.filesToSearch) {
        const files = `vec![${cmd.filesToSearch.map(file => `${goExprToRust(file)}.to_string()`).join(", ")}]`;
        return `f.verify_baseline_document_highlights_with_options(t, ${preferences}, ${files}, ${args});`;
    }
    return `f.verify_baseline_document_highlights(t, ${preferences}, ${args});`;
}

function generateRustBaselineRename(cmd: VerifyBaselineRenameCmd): string {
    if (cmd.args.some(isRangeLikeArg)) {
        return `f.verify_baseline_rename_at_marker_or_ranges(t, ${rustMarkerOrRangeVec(cmd.args)});`;
    }
    return `f.verify_baseline_rename(t, ${rustMarkerStringSlice(cmd.args)});`;
}

function rustMarkerOrRangeOrNameVec(args: string[]): string {
    if (args.length === 0 || args.some(arg => arg.includes("Ranges()") && arg.includes("ToAny"))) {
        return "f.ranges().into_iter().map(MarkerOrRangeOrName::Range).collect()";
    }
    if (args.length === 1 && args[0]! === "ToAny(f.Markers())...") {
        return "f.markers().into_iter().map(MarkerOrRangeOrName::Marker).collect()";
    }
    const items = args.map(arg => {
        if (arg.startsWith('"')) return `MarkerOrRangeOrName::Name(${goExprToRust(arg)}.to_string())`;
        const rangeMatch = arg.match(/^f\.Ranges\(\)\[(\d+)\]$/);
        if (rangeMatch) return `MarkerOrRangeOrName::Range(f.ranges()[${rangeMatch[1]}].clone())`;
        return `MarkerOrRangeOrName::Name(${goExprToRust(arg)}.to_string())`;
    });
    return `vec![${items.join(", ")}]`;
}

function rustMarkerOrRangeVec(args: string[]): string {
    if (args.length === 0) return "Vec::new()";
    if (args.length === 1) {
        const only = args[0]!;
        if (only === "ToAny(f.Ranges())...") {
            return "f.ranges().into_iter().map(Into::into).collect()";
        }
        if (only === "ToAny(f.Markers())...") {
            return "f.markers().into_iter().map(Into::into).collect()";
        }
        const rangeSlice = only.match(/^ToAny\(f\.Ranges\(\)\[(\d+):\]\)\.\.\.$/);
        if (rangeSlice) {
            return `f.ranges()[${rangeSlice[1]}..].iter().cloned().map(Into::into).collect()`;
        }
        const rangesByText = only.match(/^ToAny\(f\.GetRangesByText\(\)\.Get\((.+)\)\)\.\.\.$/);
        if (rangesByText) {
            return `f.get_ranges_by_text(${goExprToRust(rangesByText[1]!)}).into_iter().map(Into::into).collect()`;
        }
    }

    const items = args.map(arg => {
        const range = rustRangeArgToMarkerOrRange(arg);
        if (range) return range;
        if (arg.includes("ToAny(")) {
            throw new Error(`Unsupported mixed marker/range spread in baseline rename: ${arg}`);
        }
        return `f.marker_by_name(${goExprToRust(arg)}.as_str()).into()`;
    });
    return `vec![${items.join(", ")}]`;
}

function isRangeLikeArg(arg: string): boolean {
    return arg.includes("Ranges()") || arg.includes("GetRangesByText()") || arg.includes("Markers()");
}

function rustRangeArgToMarkerOrRange(arg: string): string | undefined {
    const rangeMatch = arg.match(/^f\.Ranges\(\)\[(\d+)\]$/);
    if (rangeMatch) return `f.ranges()[${rangeMatch[1]}].clone().into()`;
    const rangeByTextMatch = arg.match(/^f\.GetRangesByText\(\)\.Get\((.+)\)\[(\d+)\]$/);
    if (rangeByTextMatch) return `f.get_ranges_by_text(${goExprToRust(rangeByTextMatch[1]!)})[${rangeByTextMatch[2]}].clone().into()`;
    return undefined;
}

function generateRustNavigateTo({ args }: VerifyNavToCmd): string {
    return `f.verify_workspace_symbol(&[
${args.map(generateRustNavigateToCase).join(",\n")}
]);`;
}

function generateRustNavigateToEachRange(cmd: VerifyNavToEachRangeCmd): string {
    const pattern = rustRangeLoopPattern(cmd.pattern);
    return `for range in f.ranges() {
    f.verify_workspace_symbol(&[workspace_symbol_case_from_range_with_pattern(&range, ${pattern})]);
}`;
}

function rustRangeLoopPattern(pattern: RangeLoopNavigateToPattern): string {
    switch (pattern) {
        case "name":
            return `range_marker_data(&range).data.get("name").unwrap().to_string()`;
        case "prefixDropLast":
            return `{
        let name = range_marker_data(&range).data.get("name").unwrap();
        name[..name.len() - 1].to_string()
    }`;
        case "substringFrom2":
            return `{
        let name = range_marker_data(&range).data.get("name").unwrap();
        name[2..].to_string()
    }`;
    }
}

function generateRustNavigateToCase(arg: VerifyNavToArg): string {
    const exact = `vec![${arg.exact.length ? "\n" + arg.exact.map(generateRustNavToItem).join(",\n") + ",\n    " : ""}]`;
    if (arg.preferences) {
        return `    workspace_symbol_case_with_preferences(${rustStringLiteral(arg.pattern)}, ${exact}, ${rustNavigateToPreferences(arg.preferences)})`;
    }
    return `    workspace_symbol_case(${rustStringLiteral(arg.pattern)}, ${exact})`;
}

function generateRustNavToItem(item: NavToItem): string {
    if (item.tags) {
        throw new Error(`Unsupported navigateTo symbol tag: ${item.tags}`);
    }
    return `            symbol_information(${rustStringLiteral(item.name)}, lsproto::${item.kind}, ${rustRangeLocation(item.location)}, ${rustOptionalStrRef(item.containerName)})`;
}

function rustNavigateToPreferences(preferences: VerifyNavToPreferences | undefined): string {
    if (!preferences) return "None";
    return `Some(UserPreferences {
        exclude_library_symbols_in_nav_to: Some(${preferences.excludeLibFiles}),
        ..Default::default()
    })`;
}

function rustRangeLocation(rangeExpr: string): string {
    const rangesMatch = /^f\.Ranges\(\)\[(.+)\]$/.exec(rangeExpr);
    if (rangesMatch) return `f.ranges()[${rangesMatch[1]}].ls_location()`;
    throw new Error(`Cannot emit Rust location for range expression: ${rangeExpr}`);
}

function rustRenameFileContents(entries: RenameFileContent[]): string {
    if (entries.length === 0) return "std::collections::HashMap::<String, String>::new()";
    return `std::collections::HashMap::from([
${entries.map(entry => `    (${rustStringLiteral(entry.path)}.to_string(), ${rustRawString(entry.content)}.to_string()),`).join("\n")}
])`;
}

function generateRustImportFixAtPosition({ expectedTexts, preferences }: VerifyImportFixAtPositionCmd): string {
    const expected = expectedTexts.length === 1 && expectedTexts[0] === ""
        ? "&[]"
        : rustRawStringVecRef(expectedTexts);
    return `f.verify_import_fix_at_position(t, ${expected}, ${rustOptionalGoExpr(preferences)});`;
}

function generateRustImportFixModuleSpecifiers({ markerName, moduleSpecifiers, preferences }: VerifyImportFixModuleSpecifiersCmd): string {
    return `f.verify_import_fix_module_specifiers(t, ${rustStringLiteral(markerName)}, ${rustStringVecRef(moduleSpecifiers)}, ${rustOptionalGoExpr(preferences)});`;
}

function generateRustCodeFixAvailable({ descriptions, unavailableDescriptions, expectNone }: VerifyCodeFixAvailableCmd): string {
    if (expectNone) {
        return "f.verify_code_fix_not_available(t, &[]);";
    }
    if (unavailableDescriptions.length > 0) {
        return `f.verify_code_fix_not_available(t, ${rustStringVecRef(unavailableDescriptions)});`;
    }
    if (descriptions.length === 0) {
        return "f.verify_code_fix_available(t, None);";
    }
    return `f.verify_code_fix_available(t, Some(${rustStringVecRef(descriptions)}));`;
}

function generateRustApplyCodeActionFromCompletion({ marker, options }: VerifyApplyCodeActionFromCompletionCmd): string {
    return `f.verify_apply_code_action_from_completion(t, ${rustOptionalStrRef(marker)}, &${rustApplyCodeActionOptions(options)});`;
}

function rustApplyCodeActionOptions(options: ApplyCodeActionFromCompletionOptions): string {
    return `ApplyCodeActionFromCompletionOptions {
    name: ${rustStringLiteral(options.name)}.to_string(),
    source: ${rustStringLiteral(options.source)}.to_string(),
    auto_import_fix: ${options.autoImportFix ? "Some(AutoImportFix)" : "None"},
    description: ${rustStringLiteral(options.description)}.to_string(),
    new_file_content: ${options.newFileContent === undefined ? "None" : `Some(${rustRawString(options.newFileContent)}.to_string())`},
    new_range_content: ${options.newRangeContent === undefined ? "None" : `Some(${rustRawString(options.newRangeContent)}.to_string())`},
    user_preferences: None,
}`;
}

function rustCodeActionKind(mode: string): string {
    switch (mode) {
        case "lsproto.CodeActionKindSourceRemoveUnusedImports":
            return `"source.removeUnusedImports"`;
        case "lsproto.CodeActionKindSourceSortImports":
            return `"source.sortImports"`;
        case "lsproto.CodeActionKindSourceOrganizeImports":
        default:
            return `"source.organizeImports"`;
    }
}

function generateRustSemanticClassifications({ tokens }: VerifySemanticClassificationsCmd): string {
    const items = tokens
        .map(token => `SemanticToken { type_: ${rustStringLiteral(token.type)}.to_string(), text: ${rustStringLiteral(token.text)}.to_string() }`)
        .join(",\n");
    return `f.verify_semantic_tokens(t, &[${items}]);`;
}

function generateRustSignatureHelp({ options }: VerifySignatureHelpCmd): string {
    const lines: string[] = [];
    for (const opts of options) {
        const optionExpr = generateRustSignatureHelpOptions(opts);
        const markers = opts.marker === undefined ? [] : Array.isArray(opts.marker) ? opts.marker : [opts.marker];
        if (markers.length === 0) {
            lines.push(`f.verify_signature_help_options(t, ${optionExpr});`);
        }
        else {
            for (const marker of markers) {
                if (marker === "...test.markerNames()") {
                    lines.push(`for marker in f.marker_names() {
    f.go_to_marker(t, &marker);
    f.verify_signature_help_options(t, ${optionExpr});
}`);
                }
                else {
                    lines.push(`f.go_to_marker(t, ${rustStringLiteral(marker)});`);
                    lines.push(`f.verify_signature_help_options(t, ${optionExpr});`);
                }
            }
        }
    }
    return lines.join("\n");
}

function generateRustSignatureHelpOptions(opts: VerifySignatureHelpOptions): string {
    return `VerifySignatureHelpOptions {
    text: ${rustOptionalString(opts.text)},
    parameter_name: ${rustOptionalString(opts.parameterName)},
    parameter_span: ${rustOptionalString(opts.parameterSpan)},
    parameter_count: ${opts.parameterCount === undefined ? "None" : `Some(${opts.parameterCount})`},
    overloads_count: ${opts.overloadsCount ?? 0},
}`;
}

function generateRustNoSignatureHelp({ markers }: VerifyNoSignatureHelpCmd): string {
    if (markers.length === 1 && markers[0] === "...test.markerNames()") {
        return "f.verify_no_signature_help_for_markers(t, &f.marker_names());";
    }
    if (markers.length === 0) {
        return "f.verify_no_signature_help(t);";
    }
    return `f.verify_no_signature_help_for_markers(t, ${rustStringVecRef(markers)});`;
}

function generateRustSignatureHelpPresent({ triggerReason, markers }: VerifySignatureHelpPresentCmd): string {
    const context = generateRustTriggerContext(triggerReason);
    if (markers.length === 0) {
        return triggerReason ? `f.verify_signature_help_present_with_context(t, ${context});` : "f.verify_signature_help_present(t);";
    }
    if (triggerReason) {
        return markers.map(marker => `f.go_to_marker(t, ${rustStringLiteral(marker)});
f.verify_signature_help_present_with_context(t, ${context});`).join("\n");
    }
    return `f.verify_signature_help_present_for_markers(t, ${rustStringVecRef(markers)});`;
}

function generateRustNoSignatureHelpForTriggerReason({ triggerReason, markers }: VerifyNoSignatureHelpForTriggerReasonCmd): string {
    const context = generateRustTriggerContext(triggerReason);
    if (markers.length === 0) {
        return `f.verify_no_signature_help_with_context(t, ${context});`;
    }
    return `f.verify_no_signature_help_for_markers_with_context(t, ${rustStringVecRef(markers)}, ${context});`;
}

function generateRustTriggerContext(triggerReason: SignatureHelpTriggerReason | undefined): string {
    if (!triggerReason) return "None";
    const triggerKind = triggerReason.kind === "invoked"
        ? "lsproto::SignatureHelpTriggerKind::INVOKED"
        : "lsproto::SignatureHelpTriggerKind::TRIGGER_CHARACTER";
    return `Some(SignatureHelpContext {
    is_retrigger: ${triggerReason.kind === "retrigger"},
    trigger_character: ${rustOptionalString(triggerReason.triggerCharacter)},
    trigger_kind: Some(${triggerKind}),
})`;
}

function rustStringVecRef(values: string[]): string {
    return `&vec![${values.map(value => `${rustStringLiteral(value)}.to_string()`).join(", ")}]`;
}

function rustStringVec(values: string[]): string {
    return `vec![${values.map(value => `${rustStringLiteral(value)}.to_string()`).join(", ")}]`;
}

function rustRawStringVecRef(values: string[]): string {
    if (values.length === 0) return "&[]";
    return `&vec![${values.map(value => `${rustRawString(value)}.to_string()`).join(", ")}]`;
}

function rustOptionalString(value: string | undefined): string {
    return value === undefined ? "None" : `Some(${rustStringLiteral(value)}.to_string())`;
}

function rustOptionalStrRef(value: string | undefined): string {
    return value === undefined ? "None" : `Some(${rustStringLiteral(value)})`;
}

function generateRustEditCommand(cmd: EditCmd): string {
    switch (cmd.action) {
        case "disableFormatting":
            return `f.disable_formatting();`;
        case "insert":
            return `f.insert(t, ${rustStringLiteral(requiredString(cmd.text, cmd.action))});`;
        case "paste":
            return `f.paste(t, ${rustStringLiteral(requiredString(cmd.text, cmd.action))});`;
        case "insertLine":
            return `f.insert_line(t, ${rustStringLiteral(requiredString(cmd.text, cmd.action))});`;
        case "replaceLine":
            return `f.replace_line(t, ${requiredNumber(cmd.line, cmd.action)}, ${rustStringLiteral(requiredString(cmd.text, cmd.action))});`;
        case "backspace":
            return `f.backspace(t, ${requiredNumber(cmd.count, cmd.action)});`;
        case "deleteAtCaret":
            return `f.delete_at_caret(t, ${requiredNumber(cmd.count, cmd.action)});`;
        case "deleteLine":
            return `f.delete_line(t, ${requiredNumber(cmd.count, cmd.action)});`;
    }
}

function generateRustFormatCommand(cmd: FormatCmd): string {
    switch (cmd.action) {
        case "document":
            return `f.format_document(t, "");`;
        case "selection":
            return `f.format_selection(t, ${rustStringLiteral(requiredString(cmd.startMarker, cmd.action))}, ${rustStringLiteral(requiredString(cmd.endMarker, cmd.action))});`;
        case "setOption": {
            const option = requiredString(cmd.option, cmd.action);
            return `{
    let mut opts = f.get_options();
    opts.format_code_settings.${toSnakeCase(option)} = ${rustFormatOptionValue(option, requiredString(cmd.value, cmd.action))};
    f.configure(t, opts);
}`;
        }
    }
}

function generateRustVerifyContentCommand(cmd: VerifyContentCmd): string {
    switch (cmd.assertion) {
        case "currentFileContentIs":
            return `f.verify_current_file_content(t, ${rustRawString(cmd.text)});`;
        case "currentLineContentIs":
            return `f.verify_current_line_content(t, ${rustStringLiteral(cmd.text)});`;
        case "indentationIs":
            return `f.verify_indentation(t, ${Number(cmd.text)});`;
    }
}

function rustFormatOptionValue(option: string, value: string): string {
    if (value === "core.TSTrue") return "ts_core::TSTrue";
    if (value === "core.TSFalse") return "ts_core::TSFalse";
    if (value === "core.TSUnknown") return "ts_core::TSUnknown";
    const normalizedOption = option.toLowerCase();
    if (value === "ts.SemicolonPreference.Insert") return "lsutil::SemicolonPreference::Insert";
    if (value === "ts.SemicolonPreference.Remove") return "lsutil::SemicolonPreference::Remove";
    if (value === "ts.SemicolonPreference.Ignore") return "lsutil::SemicolonPreference::Ignore";
    if (normalizedOption === "semicolons") {
        const normalized = value.replace(/^["']|["']$/g, "").toLowerCase();
        if (normalized === "insert") return "lsutil::SemicolonPreference::Insert";
        if (normalized === "remove") return "lsutil::SemicolonPreference::Remove";
        return "lsutil::SemicolonPreference::Ignore";
    }
    if (normalizedOption === "newlinecharacter" || normalizedOption === "new_line_character") {
        return `${rustStringLiteral(value.replace(/^["']|["']$/g, ""))}.to_string()`;
    }
    return value;
}

function requiredString(value: string | undefined, context: string): string {
    if (value === undefined) throw new Error(`Missing string value for ${context}`);
    return value;
}

function requiredNumber(value: number | undefined, context: string): number {
    if (value === undefined) throw new Error(`Missing numeric value for ${context}`);
    return value;
}

function generateRustGoToCommand(cmd: GoToCmd): string {
    switch (cmd.funcName) {
        case "marker":
            return `f.go_to_marker(t, ${rustStringLiteral(requiredString(cmd.marker, cmd.funcName))});`;
        case "file":
            return `f.go_to_file(t, ${rustStringLiteral(requiredString(cmd.file, cmd.funcName))});`;
        case "fileNumber":
            return `f.go_to_file_number(t, ${requiredNumber(cmd.fileNumber, cmd.funcName)});`;
        case "position":
            return `f.go_to_position(t, ${requiredNumber(cmd.position, cmd.funcName)});`;
        case "select":
            return `f.go_to_select(t, ${rustStringLiteral(requiredString(cmd.startMarker, cmd.funcName))}, ${rustStringLiteral(requiredString(cmd.endMarker, cmd.funcName))});`;
        case "EOF":
            return `f.go_to_eof(t);`;
        case "BOF":
            return `f.go_to_bof(t);`;
    }
}

function generateRustQuickInfoCommand({ kind, marker, text, docs }: VerifyQuickInfoCmd): string {
    switch (kind) {
        case "quickInfoIs":
            return `f.verify_quick_info_is(t, ${rustStringLiteral(text ?? "")}, ${rustStringLiteral(docs ?? "")});`;
        case "quickInfoAt":
            return `f.verify_quick_info_at(t, ${rustStringLiteral(marker ?? "")}, ${rustStringLiteral(text ?? "")}, ${rustStringLiteral(docs ?? "")});`;
        case "quickInfoAtEachMarker":
            return `for marker in f.marker_names() {
    f.verify_quick_info_at(t, &marker, ${rustStringLiteral(text ?? "")}, ${rustStringLiteral(docs ?? "")});
}`;
        case "quickInfoExists":
            return "f.verify_quick_info_exists(t);";
        case "notQuickInfoExists":
            return "f.verify_not_quick_info_exists(t);";
    }
}

function generateRustVerifyCompletions({ marker, args, isNewIdentifierLocation, andApplyCodeActionArgs }: VerifyCompletionsCmd): string {
    const markerInput = rustMarkerInput(marker);
    const expectedList = rustCompletionsExpectedList(args, !!isNewIdentifierLocation);
    const call = `f.verify_completions(t, ${markerInput}, ${expectedList})`;
    if (andApplyCodeActionArgs) {
        return `${call}.and_apply_code_action(t, &CompletionsExpectedCodeAction {
    name: ${rustStringLiteral(andApplyCodeActionArgs.name)}.to_string(),
    source: ${rustStringLiteral(andApplyCodeActionArgs.source)}.to_string(),
    description: ${rustStringLiteral(andApplyCodeActionArgs.description)}.to_string(),
    new_file_content: ${rustRawString(andApplyCodeActionArgs.newFileContent)}.to_string(),
});`;
    }
    return `${call};`;
}

function rustCompletionsExpectedList(args: VerifyCompletionsArgs | "nil" | undefined, isNewIdentifierLocation: boolean): string {
    if (args === "nil") return "None";
    const expected: string[] = [];
    expected.push(`includes: ${args?.includes ? goCompletionItemsExprToRust(args.includes) : "Vec::new()"},`);
    expected.push(`excludes: ${args?.excludes ? rustStringVec(args.excludes) : "Vec::new()"},`);
    expected.push(`exact: ${args?.exact ? goCompletionItemsExprToRust(args.exact) : "Vec::new()"},`);
    expected.push(`unsorted: ${args?.unsorted ? goCompletionItemsExprToRust(args.unsorted) : "Vec::new()"},`);
    const commitCharacters = isNewIdentifierLocation ? "Some(Vec::new())" : "Some(default_commit_characters())";
    return `Some(&CompletionsExpectedList {
    is_incomplete: false,
    item_defaults: Some(CompletionsExpectedItemDefaults {
        commit_characters: ${commitCharacters},
        edit_range: ExpectedCompletionEditRange::Ignored,
    }),
    items: Some(CompletionsExpectedItems {
        ${expected.join("\n        ")}
    }),
    user_preferences: ${rustOptionalGoExpr(args?.preferences)},
})`;
}

function rustMarkerInput(marker: CompletionMarkerInput): string {
    switch (marker.kind) {
        case "none":
            return "MarkerInput::None";
        case "name":
            return `MarkerInput::Name(${rustStringLiteral(marker.name)}.to_string())`;
        case "names":
            return `MarkerInput::Names(${rustStringVec(marker.names)})`;
        case "marker":
            return `MarkerInput::Marker(f.marker_by_name(${rustStringLiteral(marker.name)}))`;
        case "allMarkers":
            return "MarkerInput::Markers(f.markers())";
    }
}

function rustOptionalGoExpr(expr: string | undefined): string {
    if (!expr || expr.startsWith("nil")) return "None";
    if (expr.includes("UserPreferences{")) {
        return `Some(${rustUserPreferencesExpr(expr)})`;
    }
    return `Some(${goExprToRust(expr)})`;
}

function rustUserPreferencesExpr(expr: string): string {
    let text = expr.trim();
    text = text.replace(/UserPreferences\{/g, "UserPreferences {");
    text = text.replace(/\blsutil\.([A-Za-z0-9_]+)/g, "lsutil::$1");
    text = text.replace(/\bcore\.([A-Za-z0-9_]+)/g, "core::$1");
    text = rustNamespaceConstants(text);
    text = rustFieldNames(text);
    return text;
}

function rustDiagnosticsArg(expr: string): string {
    if (!expr || expr.trim() === "nil" || expr.trim() === "None") {
        return "&[]";
    }
    const converted = goExprToRust(expr);
    if (converted === "None") {
        return "&[]";
    }
    if (converted.startsWith("vec![")) {
        return `&${converted}`;
    }
    return converted;
}

function goExprToRust(expr: string): string {
    let text = expr.trim();
    text = replaceGoRawStrings(text);
    text = text.replace(/\bnew\(/g, "Some(");
    text = text.replace(/\[\]string\s*\{/g, "vec![");
    text = text.replace(/\[\]fourslash\.CompletionsExpectedItem\s*\{/g, "vec![");
    text = text.replace(/\[\]CompletionsExpectedItem\s*\{/g, "vec![");
    text = text.replace(/\[\]\*lsproto\.Diagnostic\s*\{/g, "vec![");
    text = text.replace(/&lsproto\.Diagnostic\s*\{/g, "lsproto::Diagnostic {");
    text = text.replace(/\bfourslash\.CompletionsExpectedItem\{/g, "lsproto::CompletionItem {");
    text = text.replace(/\blsproto\.CompletionItem\{/g, "lsproto::CompletionItem {");
    text = text.replace(/\blsproto\.([A-Za-z0-9_]+)/g, "lsproto::$1");
    text = text.replace(/&\[\]lsproto::DiagnosticTag\{([^}]+)\}/g, "Some([$1].to_vec())");
    text = text.replace(/&\[\]lsproto::CompletionItemTag\{([^}]+)\}/g, "Some([$1].to_vec())");
    text = text.replace(/\blsutil\.([A-Za-z0-9_]+)/g, "lsutil::$1");
    text = text.replace(/\bcore\.([A-Za-z0-9_]+)/g, "core::$1");
    text = text.replace(/\bls\.([A-Za-z0-9_]+)/g, "ls::$1");
    text = text.replace(/f\.Ranges\(\)\[(\d+)\]\.FileName\(\)/g, "f.ranges()[$1].file_name()");
    text = text.replace(/f\.GetRangesByText\(\)\.Get\(([^)]+)\)\[(\d+)\]/g, (_match, key: string, index: string) => `f.get_ranges_by_text(${goExprToRust(key)})[${index}]`);
    text = text.replace(/f\.GetRangesByText\(\)\.Get\(([^)]+)\)/g, (_match, key: string) => `f.get_ranges_by_text(${goExprToRust(key)})`);
    text = rustNamespaceConstants(text);
    text = text.replace(/(?<!::)\bCompletion([A-Z][A-Za-z0-9_]*)\(/g, (_match, name: string) => `completion_${toSnakeCase(name)}(`);
    text = text.replace(/(?<!::)\bCompletion([A-Z][A-Za-z0-9_]*)\b/g, (_match, name: string) => `completion_${toSnakeCase(name)}()`);
    text = rustFieldNames(text);
    text = normalizeCompositeDelimiters(text);
    text = wrapCompletionItemStrings(text);
    text = text.replace(/\bnil\b/g, "None");
    return text;
}

function rustNamespaceConstants(text: string): string {
    return text
        .replace(/\blsproto::CompletionItemKindKeyword\b/g, "lsproto::CompletionItemKind::KEYWORD")
        .replace(/\blsproto::CompletionItemKindVariable\b/g, "lsproto::CompletionItemKind::VARIABLE")
        .replace(/\blsproto::CompletionItemKindField\b/g, "lsproto::CompletionItemKind::FIELD")
        .replace(/\blsproto::CompletionItemKindFunction\b/g, "lsproto::CompletionItemKind::FUNCTION")
        .replace(/\blsproto::CompletionItemKindMethod\b/g, "lsproto::CompletionItemKind::METHOD")
        .replace(/\blsproto::CompletionItemKindEnumMember\b/g, "lsproto::CompletionItemKind::ENUM_MEMBER")
        .replace(/\blsproto::CompletionItemKindEnum\b/g, "lsproto::CompletionItemKind::ENUM")
        .replace(/\blsproto::CompletionItemKindModule\b/g, "lsproto::CompletionItemKind::MODULE")
        .replace(/\blsproto::CompletionItemKindClass\b/g, "lsproto::CompletionItemKind::CLASS")
        .replace(/\blsproto::CompletionItemKindInterface\b/g, "lsproto::CompletionItemKind::INTERFACE")
        .replace(/\blsproto::CompletionItemKindText\b/g, "lsproto::CompletionItemKind::TEXT")
        .replace(/\blsproto::CompletionItemKindFile\b/g, "lsproto::CompletionItemKind::FILE")
        .replace(/\blsproto::CompletionItemKindFolder\b/g, "lsproto::CompletionItemKind::FOLDER")
        .replace(/\blsproto::CompletionItemKindConstant\b/g, "lsproto::CompletionItemKind::CONSTANT")
        .replace(/\blsproto::CompletionItemKindProperty\b/g, "lsproto::CompletionItemKind::PROPERTY")
        .replace(/\blsproto::DiagnosticTagUnnecessary\b/g, "lsproto::DiagnosticTag::Unnecessary")
        .replace(/\blsproto::DiagnosticTagDeprecated\b/g, "lsproto::DiagnosticTag::Deprecated")
        .replace(/\blsproto::CompletionItemTagDeprecated\b/g, "lsproto::CompletionItemTag::DEPRECATED")
        .replace(/\blsproto::InsertTextFormatSnippet\b/g, "lsproto::InsertTextFormat::Snippet")
        .replace(/\blsproto::InsertTextFormatPlainText\b/g, "lsproto::InsertTextFormat::PlainText")
        .replace(/\blsproto::SignatureHelpTriggerKind::Invoked\b/g, "lsproto::SignatureHelpTriggerKind::INVOKED")
        .replace(/\blsproto::SignatureHelpTriggerKind::TriggerCharacter\b/g, "lsproto::SignatureHelpTriggerKind::TRIGGER_CHARACTER")
        .replace(/\blsproto::SignatureHelpTriggerKind::ContentChange\b/g, "lsproto::SignatureHelpTriggerKind::CONTENT_CHANGE")
        .replace(/\blsutil::OrganizeImportsTypeOrderLast\b/g, "lsutil::OrganizeImportsTypeOrder::Last")
        .replace(/\blsutil::OrganizeImportsTypeOrderInline\b/g, "lsutil::OrganizeImportsTypeOrder::Inline")
        .replace(/\blsutil::OrganizeImportsTypeOrderFirst\b/g, "lsutil::OrganizeImportsTypeOrder::First")
        .replace(/\blsutil::OrganizeImportsCollationUnicode\b/g, "lsutil::OrganizeImportsCollation::Unicode")
        .replace(/\blsutil::OrganizeImportsCollationOrdinal\b/g, "lsutil::OrganizeImportsCollation::Ordinal")
        .replace(/\blsutil::OrganizeImportsCaseFirstUpper\b/g, "lsutil::OrganizeImportsCaseFirst::Upper")
        .replace(/\blsutil::OrganizeImportsCaseFirstLower\b/g, "lsutil::OrganizeImportsCaseFirst::Lower")
        .replace(/\blsutil::OrganizeImportsCaseFirstFalse\b/g, "lsutil::OrganizeImportsCaseFirst::False");
}

function goCompletionItemsExprToRust(expr: string): string {
    let text = convertCompletionItemLiterals(expr.trim());
    text = goExprToRust(text);
    text = text.replace(/&lsproto::([A-Za-z0-9_]+)\s*\{/g, "lsproto::$1 {");
    text = wrapBareCompletionItemStrings(text);
    if (text.startsWith("vec![")) {
        text = text.replace(/\n\s*\},?\s*$/s, "\n]");
    }
    return text;
}

function rustStringLiteral(value: string): string {
    return JSON.stringify(value);
}

function wrapCompletionItemStrings(text: string): string {
    const lines = text.split("\n");
    let mode: "completion" | "string" | undefined;
    return lines.map(line => {
        const trimmed = line.trim();
        if (/^(includes|exact|unsorted): vec!\[/.test(trimmed)) {
            mode = "completion";
            return line;
        }
        if (/^excludes: vec!\[/.test(trimmed)) {
            mode = "string";
            return line;
        }
        if (trimmed.startsWith("]")) {
            mode = undefined;
            return line;
        }
        if (mode === "completion" && /^"([^"\\]*(?:\\.[^"\\]*)*)",?$/.test(trimmed)) {
            const comma = trimmed.endsWith(",") ? "," : "";
            const value = trimmed.slice(1, trimmed.endsWith(",") ? -2 : -1);
            return line.replace(trimmed, `CompletionsExpectedItem::Label("${value}".to_string())${comma}`);
        }
        if (mode === "string" && /^"([^"\\]*(?:\\.[^"\\]*)*)",?$/.test(trimmed)) {
            const comma = trimmed.endsWith(",") ? "," : "";
            const value = trimmed.slice(1, trimmed.endsWith(",") ? -2 : -1);
            return line.replace(trimmed, `"${value}".to_string()${comma}`);
        }
        return line;
    }).join("\n");
}

function wrapBareCompletionItemStrings(text: string): string {
    const singleLine = text.match(/^vec!\[(.*)\]$/s);
    if (singleLine) {
        const items = singleLine[1]!.split(",").map(item => item.trim()).filter(Boolean);
        if (items.every(item => /^"([^"\\]*(?:\\.[^"\\]*)*)"$/.test(item))) {
            return `vec![${items.map(item => `CompletionsExpectedItem::Label(${item}.to_string())`).join(", ")}]`;
        }
    }

    return text.split("\n").map(line => {
        const trimmed = line.trim();
        if (!/^"([^"\\]*(?:\\.[^"\\]*)*)",?$/.test(trimmed)) {
            return line;
        }
        const comma = trimmed.endsWith(",") ? "," : "";
        const value = trimmed.slice(0, trimmed.endsWith(",") ? -1 : undefined);
        return line.replace(trimmed, `CompletionsExpectedItem::Label(${value}.to_string())${comma}`);
    }).join("\n");
}

function convertCompletionItemLiterals(text: string): string {
    const needle = "&lsproto.CompletionItem{";
    let result = "";
    for (let i = 0; i < text.length;) {
        const start = text.indexOf(needle, i);
        if (start < 0) {
            result += text.slice(i);
            break;
        }
        result += text.slice(i, start);
        const openBrace = start + needle.length - 1;
        const closeBrace = findMatchingBrace(text, openBrace);
        if (closeBrace < 0) {
            result += text.slice(start);
            break;
        }
        const body = text.slice(openBrace + 1, closeBrace);
        result += rustCompletionItemLiteral(body);
        i = closeBrace + 1;
    }
    return result;
}

function rustCompletionItemLiteral(body: string): string {
    const fields = splitTopLevel(body, ",")
        .map(part => part.trim())
        .filter(Boolean)
        .map(part => {
            const colon = part.indexOf(":");
            if (colon < 0) return undefined;
            const name = toSnakeCase(part.slice(0, colon).trim());
            const value = rustCompletionItemFieldValue(name, part.slice(colon + 1).trim());
            return `        ${name}: ${value},`;
        })
        .filter((part): part is string => !!part);

    return `CompletionsExpectedItem::Item(lsproto::CompletionItem {
${fields.join("\n")}
        ..Default::default()
    })`;
}

function rustCompletionItemFieldValue(name: string, value: string): string {
    if (name === "label") {
        return goStringLiteralToRustString(value) ?? goExprToRust(value);
    }
    if (["detail", "filter_text", "insert_text", "sort_text", "text_edit_text"].includes(name)) {
        const inner = value.match(/^new\((.*)\)$/s)?.[1]?.trim();
        if (inner) {
            return `Some(${goStringLiteralToRustString(inner) ?? goExprToRust(inner)})`;
        }
    }
    const rustValue = goExprToRust(value);
    return rustValue;
}

function goStringLiteralToRustString(value: string): string | undefined {
    if (/^"([^"\\]*(?:\\.[^"\\]*)*)"$/.test(value)) {
        return `${value}.to_string()`;
    }
    return undefined;
}

function findMatchingBrace(text: string, openBrace: number): number {
    let depth = 0;
    for (let i = openBrace; i < text.length; i++) {
        const ch = text[i];
        if (text.startsWith('r#"', i)) {
            i = skipRustRawString(text, i);
            continue;
        }
        if (ch === '"' || ch === "`") {
            i = skipQuoted(text, i, ch);
            continue;
        }
        if (ch === "{") depth++;
        else if (ch === "}") {
            depth--;
            if (depth === 0) return i;
        }
    }
    return -1;
}

function splitTopLevel(text: string, delimiter: string): string[] {
    const parts: string[] = [];
    let start = 0;
    let braceDepth = 0;
    let parenDepth = 0;
    let bracketDepth = 0;
    for (let i = 0; i < text.length; i++) {
        const ch = text[i];
        if (text.startsWith('r#"', i)) {
            i = skipRustRawString(text, i);
            continue;
        }
        if (ch === '"' || ch === "`") {
            i = skipQuoted(text, i, ch);
            continue;
        }
        if (ch === "{") braceDepth++;
        else if (ch === "}") braceDepth--;
        else if (ch === "(") parenDepth++;
        else if (ch === ")") parenDepth--;
        else if (ch === "[") bracketDepth++;
        else if (ch === "]") bracketDepth--;
        else if (ch === delimiter && braceDepth === 0 && parenDepth === 0 && bracketDepth === 0) {
            parts.push(text.slice(start, i));
            start = i + 1;
        }
    }
    parts.push(text.slice(start));
    return parts;
}

function skipQuoted(text: string, start: number, quote: string): number {
    for (let i = start + 1; i < text.length; i++) {
        if (quote === '"' && text[i] === "\\") {
            i++;
            continue;
        }
        if (text[i] === quote) return i;
    }
    return text.length - 1;
}

function skipRustRawString(text: string, start: number): number {
    const end = text.indexOf('"#', start + 3);
    return end < 0 ? text.length - 1 : end + 1;
}

function normalizeCompositeDelimiters(text: string): string {
    const closeStack: string[] = [];
    let result = "";
    for (let i = 0; i < text.length;) {
        if (text[i] === "`") {
            const end = text.indexOf("`", i + 1);
            if (end < 0) {
                result += text.slice(i);
                break;
            }
            result += text.slice(i, end + 1);
            i = end + 1;
            continue;
        }
        if (text[i] === '"') {
            const start = i;
            i++;
            while (i < text.length) {
                if (text[i] === "\\" && i + 1 < text.length) {
                    i += 2;
                    continue;
                }
                if (text[i] === '"') {
                    i++;
                    break;
                }
                i++;
            }
            result += text.slice(start, i);
            continue;
        }
        if (text.startsWith("vec![", i)) {
            result += "vec![";
            closeStack.push("]");
            i += "vec![".length;
            continue;
        }
        if ((text.startsWith("Some(&", i) || text.startsWith("Some(", i)) && !text.startsWith("Some(vec!", i)) {
            const brace = text.indexOf("{", i);
            const nextNewline = text.indexOf("\n", i);
            if (brace >= 0 && (nextNewline < 0 || brace < nextNewline)) {
                result += text.slice(i, brace + 1);
                closeStack.push("})");
                i = brace + 1;
                continue;
            }
        }
        if (text[i] === "{") {
            result += "{";
            closeStack.push("}");
            i++;
            continue;
        }
        if (text[i] === "}") {
            result += closeStack.pop() ?? "}";
            i++;
            continue;
        }
        result += text[i++];
    }
    return result;
}

function rustFieldNames(text: string): string {
    return text.replace(/\b([A-Z][A-Za-z0-9]*):(?=\s)/g, (_match, name: string) => `${toSnakeCase(name)}:`);
}

function replaceGoRawStrings(text: string): string {
    let result = "";
    for (let i = 0; i < text.length;) {
        if (text[i] === '"') {
            const end = skipQuoted(text, i, '"');
            result += text.slice(i, end + 1);
            i = end + 1;
            continue;
        }
        if (text[i] !== "`") {
            result += text[i++];
            continue;
        }
        const end = text.indexOf("`", i + 1);
        if (end < 0) {
            result += text.slice(i);
            break;
        }
        result += rustRawString(text.slice(i + 1, end));
        i = end + 1;
    }
    return result;
}

function rustRawStringFromGoRaw(value: string): string {
    if (!value.startsWith("`") || !value.endsWith("`")) {
        throw new Error(`Expected Go raw string literal, got ${value.slice(0, 20)}`);
    }
    return rustRawString(value.slice(1, -1).split('` + "`" + `').join("`"));
}

function rustRawString(value: string): string {
    let hashes = "";
    while (value.includes(`"${hashes}`)) {
        hashes += "#";
    }
    return `r${hashes}"${value}"${hashes}`;
}

function toSnakeCase(value: string): string {
    return value
        .replace(/([A-Z]+)([A-Z][a-z])/g, "$1_$2")
        .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
        .replace(/[^A-Za-z0-9]+/g, "_")
        .replace(/^_+|_+$/g, "")
        .toLowerCase();
}

function capitalizeIdentifier(value: string): string {
    return value.length === 0 ? value : value[0]!.toUpperCase() + value.slice(1);
}

function moduleLines(fileNames: string[]): string[] {
    const seen = new Map<string, number>();
    return fileNames.map(fileName => {
        const baseName = toSnakeCase(fileName.replace(/\.rs$/, ""));
        const count = seen.get(baseName) ?? 0;
        seen.set(baseName, count + 1);
        const moduleName = count === 0 ? baseName : `${baseName}_${count + 1}`;
        return `#[path = "${fileName}"]\npub mod ${moduleName};`;
    });
}

function testNameFromFileName(fileName: string): string {
    return fileName.replace(".tsx", "").replace(".ts", "").replace(".", "");
}

function generatedRustFileNameForTestName(testName: string): string {
    return `${toSnakeCase(testName)}_test.rs`;
}

function generatedRustFileName(testName: string): string {
    const baseName = generatedRustFileNameForTestName(testName).replace(/\.rs$/, "");
    const count = generatedFileBaseNameCounts.get(baseName) ?? 0;
    generatedFileBaseNameCounts.set(baseName, count + 1);
    return count === 0 ? `${baseName}.rs` : `${baseName}_${count + 1}.rs`;
}

function removeGeneratedRustFileForSource(sourceFileName: string): void {
    const fileName = generatedRustFileNameForTestName(testNameFromFileName(sourceFileName));
    const filePath = path.join(outputDir, fileName);
    if (fs.existsSync(filePath)) {
        fs.rmSync(filePath);
    }

    const modPath = path.join(outputDir, "mod.rs");
    if (!fs.existsSync(modPath)) {
        return;
    }
    const lines = fs.readFileSync(modPath, "utf-8").split(/\r?\n/);
    const filtered: string[] = [];
    for (let i = 0; i < lines.length; i++) {
        if (lines[i] === `#[path = "${fileName}"]`) {
            if (lines[i + 1]?.startsWith("pub mod ")) {
                i++;
            }
            continue;
        }
        filtered.push(lines[i]!);
    }
    fs.writeFileSync(modPath, filtered.join("\n").replace(/\n*$/, "\n"), "utf-8");
}

function formatGeneratedRust() {
    const files = [...generatedFiles.map(file => path.join(outputDir, file)), path.join(outputDir, "mod.rs")]
        .filter(file => fs.existsSync(file));
    for (let i = 0; i < files.length; i += 200) {
        const result = Bun.spawnSync(["rustfmt", ...files.slice(i, i + 200)], {
            stdout: "pipe",
            stderr: "pipe",
        });
        if (!result.success) {
            const stderr = new TextDecoder().decode(result.stderr);
            throw new Error(`rustfmt failed: ${stderr}`);
        }
    }
}

function getNodeOfKind<T extends ts.Node>(node: ts.Node, hasKind: (n: ts.Node) => n is T): T | undefined {
    if (hasKind(node)) {
        return node;
    }
    if (ts.isIdentifier(node)) {
        const init = getInitializer(node);
        if (init && hasKind(init)) {
            return init;
        }
    }
    return undefined;
}

function getObjectLiteralExpression(node: ts.Node): ts.ObjectLiteralExpression | undefined {
    return getNodeOfKind(node, ts.isObjectLiteralExpression);
}

function getStringLiteralLike(node: ts.Node): ts.StringLiteralLike | undefined {
    return getNodeOfKind(node, ts.isStringLiteralLike);
}

// Build a map from diagnostic property names (e.g. "Extract_base_class_to_variable")
// to their message text, by loading diagnosticMessages.json and applying the same
// key-generation algorithm used by TypeScript's processDiagnosticMessages script.
const diagnosticMessagesByPropName: Map<string, string> = (() => {
    const messagesPath = path.join(typescriptRoot, "src", "compiler", "diagnosticMessages.json");
    if (!fs.existsSync(messagesPath)) {
        return new Map<string, string>();
    }
    const raw = JSON.parse(fs.readFileSync(messagesPath, "utf-8"));
    const map = new Map<string, string>();
    for (const messageText of Object.keys(raw)) {
        const propName = messageText.split("").map((ch: string) => {
            if (ch === "*") return "_Asterisk";
            if (ch === "/") return "_Slash";
            if (ch === ":") return "_Colon";
            return /\w/.test(ch) ? ch : "_";
        }).join("")
            .replace(/_+/g, "_")
            .replace(/^_(\D)/, "$1")
            .replace(/_$/, "");
        map.set(propName, messageText);
    }
    return map;
})();

// Resolve a description value from various expression forms:
// - String literal: "Add return type 'void'"
// - ts.Diagnostics.X.message property access
// - Variable identifier referencing a const string in the same file
function resolveDescriptionExpression(expr: ts.Expression, sourceFile: ts.SourceFile): string | undefined {
    // String literal
    const str = getStringLiteralLike(expr);
    if (str) return str.text;

    // [ts.Diagnostics.Foo.message, "arg0", "arg1", ...]
    if (ts.isArrayLiteralExpression(expr) && expr.elements.length > 0) {
        const diagnostic = expr.elements[0]!;
        const template = resolveDescriptionExpression(diagnostic, sourceFile);
        if (template) {
            let message = template;
            for (let i = 1; i < expr.elements.length; i++) {
                const arg = resolveDescriptionExpression(expr.elements[i]!, sourceFile);
                if (arg === undefined) {
                    return undefined;
                }
                message = message.replaceAll(`{${i - 1}}`, arg);
            }
            return message;
        }
        return undefined;
    }

    // ts.Diagnostics.Foo_bar.message
    if (ts.isPropertyAccessExpression(expr) && expr.name.text === "message") {
        const inner = expr.expression;
        if (
            ts.isPropertyAccessExpression(inner) && ts.isPropertyAccessExpression(inner.expression)
            && ts.isIdentifier(inner.expression.name) && inner.expression.name.text === "Diagnostics"
            && ts.isIdentifier(inner.name)
        ) {
            const diagKey = inner.name.text;
            const message = diagnosticMessagesByPropName.get(diagKey);
            if (message) return message;
        }
    }

    // Variable reference: look for a const string declaration in the same file
    if (ts.isIdentifier(expr)) {
        const varName = expr.text;
        for (const stmt of sourceFile.statements) {
            if (ts.isVariableStatement(stmt)) {
                for (const decl of stmt.declarationList.declarations) {
                    if (ts.isIdentifier(decl.name) && decl.name.text === varName && decl.initializer) {
                        const initStr = getStringLiteralLike(decl.initializer);
                        if (initStr) return initStr.text;
                    }
                }
            }
        }
    }

    return undefined;
}

// Get the name of a property in an object literal, whether it's an identifier or string literal.
function getPropertyName(prop: ts.ObjectLiteralElementLike): string | undefined {
    if (ts.isPropertyAssignment(prop) || ts.isShorthandPropertyAssignment(prop)) {
        if (ts.isIdentifier(prop.name)) return prop.name.text;
        if (ts.isStringLiteral(prop.name)) return prop.name.text;
    }
    return undefined;
}

function getNumericLiteral(node: ts.Node): ts.NumericLiteral | undefined {
    return getNodeOfKind(node, ts.isNumericLiteral);
}

function isUndefinedExpression(node: ts.Node): boolean {
    return node.kind === ts.SyntaxKind.UndefinedKeyword
        || (ts.isIdentifier(node) && node.text === "undefined");
}

function getArrayLiteralExpression(node: ts.Node): ts.ArrayLiteralExpression | undefined {
    return getNodeOfKind(node, ts.isArrayLiteralExpression);
}

// Parses expressions like 'string'.length or "string".length and returns the length value
function getStringLengthExpression(node: ts.Node): number | undefined {
    if (ts.isPropertyAccessExpression(node) && node.name.text === "length") {
        const stringLiteral = getStringLiteralLike(node.expression);
        if (stringLiteral) {
            return stringLiteral.text.length;
        }
    }
    return undefined;
}

function getInitializer(name: ts.Identifier): ts.Expression | undefined {
    const file = name.getSourceFile();
    const varStmts = file.statements.filter(ts.isVariableStatement);
    for (const varStmt of varStmts) {
        const decls = varStmt.declarationList.declarations.filter(varDecl => {
            if (ts.isIdentifier(varDecl.name)) {
                return varDecl.name.text === name.text;
            }
            return false;
        });
        if (decls[0]) {
            return decls[0].initializer;
        }
    }
    return undefined;
}

if (url.fileURLToPath(import.meta.url) == process.argv[1]) {
    main().catch(e => {
        console.error(e);
        process.exit(1);
    });
}
