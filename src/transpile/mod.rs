use std::path::Path;

use anyhow::{Context, Result, bail};
use oxc::{
    allocator::Allocator,
    ast::AstKind,
    codegen::Codegen,
    diagnostics::{NamedSource, OxcDiagnostic},
    parser::Parser,
    semantic::SemanticBuilder,
    span::{GetSpan, SourceType},
    transformer::{TransformOptions, Transformer},
};

pub(crate) fn transpile(path: &Path, source: &str) -> Result<String> {
    // Parse it.
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::ts()).parse();
    diagnostics_to_result(path, source, parsed.diagnostics).context("parse")?;
    let mut program = parsed.program;

    // Check the semantic.
    let semantic = SemanticBuilder::new_compiler()
        .with_build_nodes(true)
        .with_enum_eval(true)
        .build(&program);
    diagnostics_to_result(path, source, semantic.diagnostics).context("semantic analysis")?;

    // Check all `import` or `export` statement and make sure it only import
    // from snek.
    let unsupported = semantic.semantic.nodes().iter().filter_map(|node| {
        let kind = node.kind();
        let unsupported = match kind {
            AstKind::ImportDeclaration(import) => import.source.value != "snek",
            _ => {
                let is_module_declaration_node = kind.is_module_declaration();
                let is_other_import_node = matches!(
                    kind,
                    AstKind::ImportExpression(_)
                        | AstKind::ImportMeta(_)
                        | AstKind::TSImportEqualsDeclaration(_)
                        | AstKind::TSImportType(_)
                );
                is_module_declaration_node || is_other_import_node
            }
        };
        unsupported.then(|| {
            OxcDiagnostic::error("only static imports from \"snek\" are supported")
                .with_label(kind.span())
        })
    });
    diagnostics_to_result(path, source, unsupported).context("script validation")?;

    // Transform it to plain JavaScript program.
    let mut options = TransformOptions::default();
    options.typescript.only_remove_type_imports = true;
    let transformed = Transformer::new(&allocator, path, &options)
        .build_with_scoping(semantic.semantic.into_scoping(), &mut program);
    diagnostics_to_result(path, source, transformed.diagnostics).context("transform")?;

    // Codegen the source string from the program and return.
    Ok(Codegen::new().build(&program).code)
}

fn diagnostics_to_result(
    path: &Path,
    source: &str,
    diagnostics: impl IntoIterator<Item = OxcDiagnostic>,
) -> Result<()> {
    let mut diagnostics = diagnostics.into_iter().peekable();
    if diagnostics.peek().is_none() {
        return Ok(());
    }

    let mut reports = String::new();
    for diagnostic in diagnostics {
        let source = NamedSource::new(path.to_string_lossy(), source);
        reports.push_str(&diagnostic.render_with_source_code(source));
    }
    bail!("\n{reports}")
}
