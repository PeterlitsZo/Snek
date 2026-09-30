use std::path::Path;

use anyhow::{Result, bail};
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
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::ts().with_script(true)).parse();
    check_diagnostics(path, source, "parse", parsed.diagnostics)?;
    let mut program = parsed.program;

    let semantic = SemanticBuilder::new_compiler()
        .with_build_nodes(true)
        .with_enum_eval(true)
        .build(&program);
    check_diagnostics(path, source, "semantic analysis", semantic.diagnostics)?;

    // Type-only and unused imports can disappear during transformation. Reject
    // module syntax beforehand, including dynamic imports inside functions.
    let unsupported = semantic.semantic.nodes().iter().filter_map(|node| {
        let kind = node.kind();
        (kind.is_module_declaration()
            || matches!(
                kind,
                AstKind::ImportExpression(_)
                    | AstKind::ImportMeta(_)
                    | AstKind::TSImportEqualsDeclaration(_)
                    | AstKind::TSImportType(_)
            ))
        .then(|| {
            OxcDiagnostic::error("module syntax is not supported; use a single-file script")
                .with_label(kind.span())
        })
    });
    check_diagnostics(path, source, "script validation", unsupported)?;

    // Default options strip TypeScript without lowering modern JavaScript to an
    // older target or enabling the example's unrelated transform plugins.
    let options = TransformOptions::default();
    let transformed = Transformer::new(&allocator, path, &options)
        .build_with_scoping(semantic.semantic.into_scoping(), &mut program);
    check_diagnostics(path, source, "transform", transformed.diagnostics)?;

    Ok(Codegen::new().build(&program).code)
}

fn check_diagnostics(
    path: &Path,
    source: &str,
    stage: &str,
    diagnostics: impl IntoIterator<Item = OxcDiagnostic>,
) -> Result<()> {
    let mut reports = String::new();
    for diagnostic in diagnostics {
        let source = NamedSource::new(path.to_string_lossy(), source);
        reports.push_str(&diagnostic.render_with_source_code(source));
    }
    if !reports.is_empty() {
        bail!("{stage}:\n{reports}");
    }
    Ok(())
}
